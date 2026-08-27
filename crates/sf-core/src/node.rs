use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};

use thiserror::Error;

use crate::SessionStatusObserver;
use crate::{CallerId, Session, SessionId, SessionState, TransferDirection, TransportKind};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeId(NonZeroU32);

impl NodeId {
    pub fn new(value: u32) -> Result<Self, NodeError> {
        NonZeroU32::new(value)
            .map(Self)
            .ok_or(NodeError::InvalidNodeNumber(value))
    }

    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeDefinition {
    pub id: NodeId,
    pub enabled: bool,
    pub description: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeRuntimeState {
    Waiting,
    Connecting,
    Login,
    Online,
    PagePending,
    Chatting,
    Downloading,
    Uploading,
    Disconnecting,
    Disabled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeSnapshot {
    pub id: NodeId,
    pub description: Option<String>,
    pub state: NodeRuntimeState,
    pub session_id: Option<SessionId>,
    pub caller_id: Option<CallerId>,
    pub caller_name: Option<String>,
    pub transport: Option<TransportKind>,
    pub connected_at: Option<i64>,
    pub activity_file: Option<String>,
    pub presentation: Option<NodePresentationContext>,
}

/// Privacy-bounded, read-only presentation facts for one active node. These
/// values are observed from the session engine and cannot grant commands or
/// alter terminal, profile, locale, or security state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodePresentationContext {
    pub terminal_type: Option<String>,
    pub ansi: bool,
    pub encoding: String,
    pub columns: Option<u16>,
    pub rows: Option<u16>,
    pub page_length: Option<u16>,
    pub locale: String,
    pub presentation_profile: String,
    pub menu_mode: String,
    pub menu_context: Option<String>,
    pub renderer_path: Option<MenuRendererPath>,
    pub caller_security: Option<u16>,
    pub sysop_threshold: u16,
    pub visible_action_count: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuRendererPath {
    ExactSecurityBoardOverride,
    ExactSecurityActiveProfile,
    GeneratedStock,
    ExpertSuppressed,
}

impl MenuRendererPath {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExactSecurityBoardOverride => "exact-security-board-override",
            Self::ExactSecurityActiveProfile => "exact-security-active-profile",
            Self::GeneratedStock => "generated-stock",
            Self::ExpertSuppressed => "expert-suppressed",
        }
    }
}

struct NodeSlot {
    definition: NodeDefinition,
    state: NodeRuntimeState,
    session_id: Option<SessionId>,
    caller_id: Option<CallerId>,
    caller_name: Option<String>,
    transport: Option<TransportKind>,
    connected_at: Option<i64>,
    activity_file: Option<String>,
    presentation: Option<NodePresentationContext>,
}

impl NodeSlot {
    fn snapshot(&self) -> NodeSnapshot {
        NodeSnapshot {
            id: self.definition.id,
            description: self.definition.description.clone(),
            state: self.state,
            session_id: self.session_id,
            caller_id: self.caller_id,
            caller_name: self.caller_name.clone(),
            transport: self.transport,
            connected_at: self.connected_at,
            activity_file: self.activity_file.clone(),
            presentation: self.presentation.clone(),
        }
    }

    fn clear(&mut self) {
        self.state = if self.definition.enabled {
            NodeRuntimeState::Waiting
        } else {
            NodeRuntimeState::Disabled
        };
        self.session_id = None;
        self.caller_id = None;
        self.caller_name = None;
        self.transport = None;
        self.connected_at = None;
        self.activity_file = None;
        self.presentation = None;
    }
}

pub type NodeChangeHook = Arc<dyn Fn(&[NodeSnapshot]) + Send + Sync>;

struct NodeManagerInner {
    nodes: Mutex<Vec<NodeSlot>>,
    change_hook: Option<NodeChangeHook>,
    notify_lock: Mutex<()>,
}

/// Race-safe configured node pool shared by every terminal transport.
#[derive(Clone)]
pub struct NodeManager {
    inner: Arc<NodeManagerInner>,
}

impl NodeManager {
    pub fn new(definitions: Vec<NodeDefinition>) -> Result<Self, NodeError> {
        Self::with_change_hook(definitions, None)
    }

    pub fn with_change_hook(
        definitions: Vec<NodeDefinition>,
        change_hook: Option<NodeChangeHook>,
    ) -> Result<Self, NodeError> {
        if definitions.is_empty() {
            return Err(NodeError::NoConfiguredNodes);
        }
        let mut previous = None;
        let mut nodes = Vec::with_capacity(definitions.len());
        for definition in definitions {
            if previous.is_some_and(|value| value >= definition.id) {
                return Err(NodeError::DefinitionsNotStrictlyOrdered);
            }
            previous = Some(definition.id);
            nodes.push(NodeSlot {
                state: if definition.enabled {
                    NodeRuntimeState::Waiting
                } else {
                    NodeRuntimeState::Disabled
                },
                definition,
                session_id: None,
                caller_id: None,
                caller_name: None,
                transport: None,
                connected_at: None,
                activity_file: None,
                presentation: None,
            });
        }
        let manager = Self {
            inner: Arc::new(NodeManagerInner {
                nodes: Mutex::new(nodes),
                change_hook,
                notify_lock: Mutex::new(()),
            }),
        };
        manager.notify()?;
        Ok(manager)
    }

    pub fn acquire(
        &self,
        session_id: SessionId,
        transport: TransportKind,
        connected_at: i64,
    ) -> Result<NodeLease, NodeError> {
        let node_id = {
            let mut nodes = self.lock()?;
            let Some(node) = nodes
                .iter_mut()
                .find(|node| node.state == NodeRuntimeState::Waiting)
            else {
                return Err(NodeError::AllNodesBusy);
            };
            node.state = NodeRuntimeState::Connecting;
            node.session_id = Some(session_id);
            node.transport = Some(transport);
            node.connected_at = Some(connected_at);
            node.definition.id
        };
        self.notify()?;
        Ok(NodeLease {
            manager: self.clone(),
            node_id,
            session_id,
            released: false,
        })
    }

    pub fn snapshots(&self) -> Result<Vec<NodeSnapshot>, NodeError> {
        Ok(self.lock()?.iter().map(NodeSlot::snapshot).collect())
    }

    pub fn available(&self) -> Result<usize, NodeError> {
        Ok(self
            .lock()?
            .iter()
            .filter(|node| node.state == NodeRuntimeState::Waiting)
            .count())
    }

    fn update(
        &self,
        node_id: NodeId,
        session_id: SessionId,
        state: NodeRuntimeState,
        caller: Option<(CallerId, &str)>,
    ) -> Result<(), NodeError> {
        {
            let mut nodes = self.lock()?;
            let node = nodes
                .iter_mut()
                .find(|node| node.definition.id == node_id)
                .ok_or(NodeError::UnknownNode(node_id.get()))?;
            if node.session_id != Some(session_id) {
                return Err(NodeError::SessionMismatch {
                    node: node_id.get(),
                    expected: node.session_id.map(SessionId::get),
                    actual: session_id.get(),
                });
            }
            node.state = state;
            if !matches!(
                state,
                NodeRuntimeState::Downloading
                    | NodeRuntimeState::Uploading
                    | NodeRuntimeState::PagePending
                    | NodeRuntimeState::Chatting
            ) {
                node.activity_file = None;
            }
            if let Some((caller_id, caller_name)) = caller {
                node.caller_id = Some(caller_id);
                node.caller_name = Some(caller_name.to_owned());
            }
        }
        self.notify()
    }

    fn update_transfer(
        &self,
        node_id: NodeId,
        session_id: SessionId,
        state: NodeRuntimeState,
        filename: &str,
    ) -> Result<(), NodeError> {
        {
            let mut nodes = self.lock()?;
            let node = nodes
                .iter_mut()
                .find(|node| node.definition.id == node_id)
                .ok_or(NodeError::UnknownNode(node_id.get()))?;
            if node.session_id != Some(session_id) {
                return Err(NodeError::SessionMismatch {
                    node: node_id.get(),
                    expected: node.session_id.map(SessionId::get),
                    actual: session_id.get(),
                });
            }
            node.state = state;
            node.activity_file = Some(filename.to_owned());
        }
        self.notify()
    }

    fn update_presentation(
        &self,
        node_id: NodeId,
        session_id: SessionId,
        presentation: NodePresentationContext,
    ) -> Result<(), NodeError> {
        {
            let mut nodes = self.lock()?;
            let node = nodes
                .iter_mut()
                .find(|node| node.definition.id == node_id)
                .ok_or(NodeError::UnknownNode(node_id.get()))?;
            if node.session_id != Some(session_id) {
                return Err(NodeError::SessionMismatch {
                    node: node_id.get(),
                    expected: node.session_id.map(SessionId::get),
                    actual: session_id.get(),
                });
            }
            node.presentation = Some(presentation);
        }
        self.notify()
    }

    fn release(&self, node_id: NodeId, session_id: SessionId) -> Result<(), NodeError> {
        {
            let mut nodes = self.lock()?;
            let node = nodes
                .iter_mut()
                .find(|node| node.definition.id == node_id)
                .ok_or(NodeError::UnknownNode(node_id.get()))?;
            if node.session_id != Some(session_id) {
                return Err(NodeError::SessionMismatch {
                    node: node_id.get(),
                    expected: node.session_id.map(SessionId::get),
                    actual: session_id.get(),
                });
            }
            node.clear();
        }
        self.notify()
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Vec<NodeSlot>>, NodeError> {
        self.inner
            .nodes
            .lock()
            .map_err(|_| NodeError::CoordinationPoisoned)
    }

    fn notify(&self) -> Result<(), NodeError> {
        let _notification = self
            .inner
            .notify_lock
            .lock()
            .map_err(|_| NodeError::CoordinationPoisoned)?;
        if let Some(hook) = &self.inner.change_hook {
            let snapshots = self.snapshots()?;
            hook(&snapshots);
        }
        Ok(())
    }
}

/// RAII ownership of one configured node. Dropping a lease releases the node
/// even when a transport or session returns early with an error.
pub struct NodeLease {
    manager: NodeManager,
    node_id: NodeId,
    session_id: SessionId,
    released: bool,
}

impl NodeLease {
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    pub fn start_session(&self) -> Session {
        Session::new(self.session_id, self.node_id)
    }

    pub fn mark_login(&self) -> Result<(), NodeError> {
        self.manager
            .update(self.node_id, self.session_id, NodeRuntimeState::Login, None)
    }

    pub fn mark_online(&self, caller_id: CallerId, caller_name: &str) -> Result<(), NodeError> {
        self.manager.update(
            self.node_id,
            self.session_id,
            NodeRuntimeState::Online,
            Some((caller_id, caller_name)),
        )
    }

    pub fn mark_disconnecting(&self) -> Result<(), NodeError> {
        self.manager.update(
            self.node_id,
            self.session_id,
            NodeRuntimeState::Disconnecting,
            None,
        )
    }

    pub fn release(mut self, session: &Session) -> Result<(), NodeError> {
        if session.id() != self.session_id || session.node_id() != self.node_id {
            return Err(NodeError::LeaseSessionMismatch);
        }
        if session.state() != SessionState::Closed {
            return Err(NodeError::SessionNotClosed {
                node: self.node_id.get(),
                session: self.session_id.get(),
            });
        }
        self.manager.release(self.node_id, self.session_id)?;
        self.released = true;
        Ok(())
    }
}

impl SessionStatusObserver for NodeLease {
    fn login_started(&self) -> Result<(), NodeError> {
        self.mark_login()
    }

    fn caller_authenticated(
        &self,
        caller_id: CallerId,
        caller_name: &str,
    ) -> Result<(), NodeError> {
        self.mark_online(caller_id, caller_name)
    }

    fn transfer_started(
        &self,
        direction: TransferDirection,
        filename: &str,
    ) -> Result<(), NodeError> {
        self.manager.update_transfer(
            self.node_id,
            self.session_id,
            match direction {
                TransferDirection::Download => NodeRuntimeState::Downloading,
                TransferDirection::Upload => NodeRuntimeState::Uploading,
            },
            filename,
        )
    }

    fn transfer_finished(&self) -> Result<(), NodeError> {
        self.manager.update(
            self.node_id,
            self.session_id,
            NodeRuntimeState::Online,
            None,
        )
    }

    fn page_pending(&self) -> Result<(), NodeError> {
        self.manager.update(
            self.node_id,
            self.session_id,
            NodeRuntimeState::PagePending,
            None,
        )
    }

    fn chat_started(&self) -> Result<(), NodeError> {
        self.manager.update(
            self.node_id,
            self.session_id,
            NodeRuntimeState::Chatting,
            None,
        )
    }

    fn interaction_finished(&self) -> Result<(), NodeError> {
        self.manager.update(
            self.node_id,
            self.session_id,
            NodeRuntimeState::Online,
            None,
        )
    }

    fn presentation_changed(&self, presentation: NodePresentationContext) -> Result<(), NodeError> {
        self.manager
            .update_presentation(self.node_id, self.session_id, presentation)
    }
}

impl Drop for NodeLease {
    fn drop(&mut self) {
        if !self.released {
            let _ = self.manager.release(self.node_id, self.session_id);
        }
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum NodeError {
    #[error("node number must be between 1 and 4294967295, got {0}")]
    InvalidNodeNumber(u32),
    #[error("at least one configured node is required")]
    NoConfiguredNodes,
    #[error("node definitions must be unique and strictly ordered")]
    DefinitionsNotStrictlyOrdered,
    #[error("all enabled SPITFIRE nodes are busy")]
    AllNodesBusy,
    #[error("configured node {0} does not exist")]
    UnknownNode(u32),
    #[error("node {node} belongs to session {expected:?}, not session {actual}")]
    SessionMismatch {
        node: u32,
        expected: Option<u64>,
        actual: u64,
    },
    #[error("node {node} cannot release session {session} before it closes")]
    SessionNotClosed { node: u32, session: u64 },
    #[error("node lease does not belong to the supplied session")]
    LeaseSessionMismatch,
    #[error("node-manager coordination lock was poisoned")]
    CoordinationPoisoned,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_paths_distinguish_board_profile_generated_and_expert_sources() {
        assert_eq!(
            MenuRendererPath::ExactSecurityBoardOverride.as_str(),
            "exact-security-board-override"
        );
        assert_eq!(
            MenuRendererPath::ExactSecurityActiveProfile.as_str(),
            "exact-security-active-profile"
        );
        assert_eq!(MenuRendererPath::GeneratedStock.as_str(), "generated-stock");
        assert_eq!(
            MenuRendererPath::ExpertSuppressed.as_str(),
            "expert-suppressed"
        );
    }
    use crate::SessionCloseReason;
    use std::sync::{Arc, Barrier};
    use std::thread;

    fn definitions(count: u32) -> Vec<NodeDefinition> {
        (1..=count)
            .map(|number| NodeDefinition {
                id: NodeId::new(number).unwrap(),
                enabled: true,
                description: Some(format!("Node {number}")),
            })
            .collect()
    }

    #[test]
    fn allocates_lowest_available_node_and_reuses_it_after_release() {
        let manager = NodeManager::new(definitions(2)).unwrap();
        let first = manager
            .acquire(SessionId::new(1).unwrap(), TransportKind::Telnet, 10)
            .unwrap();
        let second = manager
            .acquire(SessionId::new(2).unwrap(), TransportKind::RawTcp, 11)
            .unwrap();
        assert_eq!(first.node_id().get(), 1);
        assert_eq!(second.node_id().get(), 2);
        assert!(matches!(
            manager.acquire(SessionId::new(3).unwrap(), TransportKind::Rlogin, 12),
            Err(NodeError::AllNodesBusy)
        ));
        let mut session = first.start_session();
        session.activate().unwrap();
        session.close(SessionCloseReason::Goodbye).unwrap();
        first.release(&session).unwrap();
        let replacement = manager
            .acquire(SessionId::new(3).unwrap(), TransportKind::Rlogin, 12)
            .unwrap();
        assert_eq!(replacement.node_id().get(), 1);
        drop(second);
        drop(replacement);
        assert_eq!(manager.available().unwrap(), 2);
    }

    #[test]
    fn concurrent_acquisition_never_assigns_one_node_twice() {
        let manager = NodeManager::new(definitions(4)).unwrap();
        let barrier = Arc::new(Barrier::new(9));
        let mut handles = Vec::new();
        for number in 1..=8 {
            let manager = manager.clone();
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier.wait();
                manager.acquire(
                    SessionId::new(number).unwrap(),
                    TransportKind::RawTcp,
                    number as i64,
                )
            }));
        }
        barrier.wait();
        let leases: Vec<_> = handles
            .into_iter()
            .filter_map(|handle| handle.join().unwrap().ok())
            .collect();
        assert_eq!(leases.len(), 4);
        let mut ids: Vec<_> = leases.iter().map(|lease| lease.node_id().get()).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec![1, 2, 3, 4]);
    }

    #[test]
    fn disabled_nodes_are_reported_but_never_allocated() {
        let mut configured = definitions(2);
        configured[0].enabled = false;
        let manager = NodeManager::new(configured).unwrap();
        let lease = manager
            .acquire(SessionId::new(1).unwrap(), TransportKind::Telnet, 10)
            .unwrap();
        assert_eq!(lease.node_id().get(), 2);
        assert_eq!(
            manager.snapshots().unwrap()[0].state,
            NodeRuntimeState::Disabled
        );
    }

    #[test]
    fn transfer_activity_is_published_without_exposing_a_storage_path() {
        let manager = NodeManager::new(definitions(1)).unwrap();
        let lease = manager
            .acquire(SessionId::new(1).unwrap(), TransportKind::Telnet, 10)
            .unwrap();
        let caller = CallerId::new(7).unwrap();
        lease.mark_online(caller, "File Caller").unwrap();
        lease
            .transfer_started(TransferDirection::Download, "WELCOME.TXT")
            .unwrap();
        let active = manager.snapshots().unwrap().remove(0);
        assert_eq!(active.state, NodeRuntimeState::Downloading);
        assert_eq!(active.activity_file.as_deref(), Some("WELCOME.TXT"));
        lease.transfer_finished().unwrap();
        let finished = manager.snapshots().unwrap().remove(0);
        assert_eq!(finished.state, NodeRuntimeState::Online);
        assert_eq!(finished.activity_file, None);
    }

    #[test]
    fn presentation_diagnostics_are_node_local_and_cleared_with_the_lease() {
        let manager = NodeManager::new(definitions(2)).unwrap();
        let first = manager
            .acquire(SessionId::new(1).unwrap(), TransportKind::Telnet, 10)
            .unwrap();
        let second = manager
            .acquire(SessionId::new(2).unwrap(), TransportKind::RawTcp, 11)
            .unwrap();
        first
            .presentation_changed(NodePresentationContext {
                terminal_type: Some("ANSI".to_owned()),
                ansi: true,
                encoding: "cp437".to_owned(),
                columns: Some(80),
                rows: Some(25),
                page_length: Some(24),
                locale: "en-US".to_owned(),
                presentation_profile: "modern-ng".to_owned(),
                menu_mode: "generated".to_owned(),
                menu_context: Some("main".to_owned()),
                renderer_path: Some(MenuRendererPath::GeneratedStock),
                caller_security: Some(10),
                sysop_threshold: 50,
                visible_action_count: Some(11),
            })
            .unwrap();
        let active = manager.snapshots().unwrap();
        assert_eq!(active[0].presentation.as_ref().unwrap().columns, Some(80));
        assert_eq!(active[1].presentation, None);
        drop(first);
        assert_eq!(manager.snapshots().unwrap()[0].presentation, None);
        drop(second);
    }

    #[test]
    fn rejects_node_zero() {
        assert_eq!(NodeId::new(0).unwrap_err(), NodeError::InvalidNodeNumber(0));
    }
}
