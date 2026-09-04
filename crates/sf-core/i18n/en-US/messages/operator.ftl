# SPITFIRE NG canonical en-US interactive operator catalog.
operator-usage = Usage:
    spitfire [--locale <BCP47>] --version
    spitfire [--locale <BCP47>] init-fixture <OUTPUT-DIRECTORY>
    spitfire [--locale <BCP47>] setup <OUTPUT-DIRECTORY>
    spitfire [--locale <BCP47>] config <CONFIG-FILE>
    spitfire [--locale <BCP47>] status <CONFIG-FILE>
    spitfire [--locale <BCP47>] backup <CONFIG-FILE> <BACKUP-DIRECTORY>
    spitfire [--locale <BCP47>] restore <BACKUP-DIRECTORY> <BOARD-DIRECTORY> [--replace]
    spitfire [--locale <BCP47>] init-sysop <CONFIG-FILE>
    spitfire [--locale <BCP47>] demo <CONFIG-FILE>
    spitfire [--locale <BCP47>] shell <CONFIG-FILE>
    spitfire [--locale <BCP47>] console <CONFIG-FILE>
    spitfire [--locale <BCP47>] run <CONFIG-FILE> [--max-sessions <COUNT>]
operator-version = SPITFIRE NG Bulletin Board System { $version }
operator-language-valid = Language package { $locale } { $version } is valid and was not installed.
operator-language-installed = Installed language package { $locale } { $version }. Select it with `spitfire config { $config }`.
operator-setup-complete = SPITFIRE NG board setup complete.
    Configuration: { $config }
    Database: { $database }
    Schema version: { $schema }
    Configured nodes: { $nodes }
    Message conferences: { $conferences }
    File areas: { $areas }
    Sysop caller: { $sysop }
    Start with: spitfire run { $config }
operator-backup-complete = SPITFIRE NG cold backup complete.
    Board: { $board }
    Destination: { $destination }
    Schema version: { $schema }
    Resources: { $resources }
    Cataloged files: { $files }
    Verified bytes: { $bytes }
    Contents: exact configuration, SQLite operational state, SYSTEM/DISPLAY resources, and cataloged file bytes. Runtime status and incomplete upload staging are excluded.
operator-restore-complete = SPITFIRE NG cold restore complete.
    Board: { $board }
    Root: { $root }
    Configuration: { $config }
    Schema version: { $schema }
    Resources: { $resources }
    Cataloged files: { $files }
    Existing board replaced: { $replaced }
operator-fixture-complete = Initialized synthetic fixture board.
    Configuration: { $config }
    Database: { $database }
    Schema version: { $schema }
    No default Sysop password was created. Run `spitfire init-sysop { $config }` to initialize it explicitly.
operator-init-sysop-password = New SPITFIRE Sysop password: { " " }
operator-init-sysop-complete = Initialized SPITFIRE Sysop caller { $caller } with security level { $security }. No default password was stored.
operator-demo-summary = Runtime summary: board={ $board }, node={ $node }, schema={ $schema }, session={ $session }, caller={ $caller }, transport={ $transport }, commands={ $commands }, shutdown=clean
operator-shell-ended = SPITFIRE shell session { $session } ended { $reason }.
operator-console-ended = SPITFIRE operator console stopped after { $sessions } completed sessions.
operator-listeners-ended = SPITFIRE listeners stopped after { $sessions } completed sessions.
operator-setup-title = SPITFIRE NG Board Setup
operator-setup-profile-summary = Installed presentation profiles: modern-ng, minimal-terminal, classic-spitfire.
operator-setup-board-name = Board name
operator-setup-sysop-display-name = Sysop display name
operator-setup-sysop-caller-name = Sysop caller name
operator-setup-node-count = Number of nodes
operator-setup-timezone-help = The board timezone controls local dates and daily limits. Use an IANA name such as America/Phoenix, America/New_York, or Europe/London.
operator-setup-timezone = Board timezone (IANA name)
operator-setup-board-access = Board access (public/private)
operator-setup-private-security = Minimum pre-authorized private-board security
operator-setup-presentation-help = Presentation, menu source, and post-login journey are separate settings.
operator-setup-experience-preset = Caller experience preset (modern/classic/minimal/custom)
operator-setup-active-profile = Active presentation profile
operator-setup-base-profile = Base presentation profile
operator-setup-menu-presentation = Menu presentation (display-overrides/generated)
operator-setup-post-login = Post-login journey (none/stock)
operator-setup-new-caller-security = New-caller security level
operator-setup-sysop-threshold = Sysop security threshold
operator-setup-initial-sysop-security = Initial Sysop caller security level
operator-setup-default-locale = Board default locale (BCP 47)
operator-setup-minutes-call = Minutes per call
operator-setup-minutes-day = Minutes per day
operator-setup-new-caller-minutes = New-caller first-day minutes
operator-setup-calls-day = Maximum calls per board-local day
operator-setup-inactivity = No-activity time limit in minutes
operator-setup-address-policy = Postal address policy
operator-setup-phone-policy = Phone policy
operator-setup-email-policy = Email policy
operator-setup-birthday-policy = Birth-date policy
operator-setup-policy-options = { $label } (disabled/optional/required)
operator-setup-enable-listener = Enable { $listener } (yes/no)
operator-setup-listener-bind = { $listener } bind address
operator-setup-listener-port = { $listener } port
operator-setup-yes-no-invalid = Please enter yes/y or no/n.
operator-setup-password-length = Password must contain { $minimum }..={ $maximum } bytes. Please try again.
operator-setup-password-mismatch = Passwords did not match. Please try again.
operator-setup-password = Initial Sysop password: { " " }
operator-setup-password-confirm = Confirm password: { " " }
operator-config-title = SPITFIRE NG CONFIGURATION
    1 General System / Sysop Identity
    2 Nodes
    3 Terminal Services
    4 Security / Caller Defaults
    5 Message Conferences
    6 File Areas
    7 Presentation Profile
    8 Language / Locale
    S Save Static Configuration
    Q Quit
operator-config-selection = Selection
operator-config-node-count = Configured node count
operator-config-presentation-mode = Presentation mode (profile/legacy-resources)
operator-config-active-profile = Active profile ID
operator-config-base-profile = Base profile ID
operator-config-presentation-invalid = Presentation mode must be profile or legacy-resources.
operator-config-saved = Static configuration saved and validated.
operator-config-ended = SPITFIRE configuration session ended.
operator-config-unknown = Unknown configuration selection.
operator-config-file-areas-title = File Areas (changes are immediate):
operator-config-file-area-row =   { $number } { $name } active={ $active } files={ $files } read={ $read } upload={ $upload } storage={ $storage }
operator-config-edit-actions = [A]dd, [E]dit, [T]oggle, Enter to return
operator-config-file-area-number = File area number
operator-config-file-area-name = Area description/name
operator-config-file-area-description = Long description
operator-config-file-area-storage = Storage key (safe relative name; immutable after creation)
operator-config-file-area-security = Area security
operator-config-file-area-upload-security = Upload security
operator-config-file-area-exact = Require exact area security (yes/no)
operator-config-file-area-preview = Allow lower-security preview (yes/no)
operator-config-file-area-free = No-charge/free area (yes/no)
operator-config-file-area-max-mib = Maximum upload MiB
operator-config-privileged-levels = Privileged security levels (comma-separated, up to five)
operator-config-services-title = Configured Terminal Services:
operator-config-service-row =   { $number } { $name } enabled={ $enabled } endpoint={ $endpoint }
operator-config-service-selection = Service number to edit (blank to return)
operator-config-enabled = Enabled (yes/no)
operator-config-bind = Bind address and port
operator-config-conferences-title = Message Conferences (changes are immediate):
operator-config-conference-row =   { $number } { $name } active={ $active } read={ $read } post={ $post } mode={ $mode }
operator-config-conference-number = Conference number
operator-config-conference-name = Conference name
operator-config-description = Description
operator-config-read-security = Read security
operator-config-post-security = Message entry security
operator-config-exact-read = Require exact read security (yes/no)
operator-config-public-only = Public messages only (yes/no)
operator-config-caller-message-deletion = Allow caller message deletion (yes/no)
operator-config-maximum-lines = Maximum message lines
operator-prompt-return = return
operator-status-language-title = Language:
operator-status-default-locale = Default locale: { $locale }
operator-status-effective-locale = Effective locale: { $locale }
operator-status-language-package = Package: { $locale } { $version }
operator-status-language-state = Status: { $status }
operator-status-language-issue = Issue: { $issue }
operator-status-ready = ready
operator-status-degraded = DEGRADED
operator-status-header = SPITFIRE NG STATUS
operator-status-software = Software: SPITFIRE NG Bulletin Board System { $version }
operator-status-board = Board: { $board }
operator-status-sysop = Sysop: { $sysop }
operator-status-configuration = Configuration: { $path }
operator-status-runtime = Runtime: { $state }
operator-status-runtime-published = published/running or not cleanly stopped
operator-status-runtime-offline = offline
operator-status-presentation-title = Presentation:
operator-status-public-information = Public information: directory={ $enabled } last-call={ $last_call } location={ $location } caller-additions={ $caller_additions } version={ $version }
operator-status-presentation =   Mode: { $mode }
    Menu presentation: { $menu }
    Active: { $active }
    Base: { $base }
    Effective: { $effective }
    Status: { $status }
operator-status-post-login = Post-login journey: { $journey }
operator-status-new-caller-security = New-caller security: { $security }
operator-status-sysop-security = Sysop security threshold: { $security }
operator-status-terminal-services = Terminal Services:
operator-status-listener =   { $name } { $transport } enabled={ $enabled } { $endpoint }
operator-status-ssh-host-key =   SSH host key: { $path } fingerprint={ $fingerprint }
operator-status-nodes = Nodes:
operator-status-published-times = Published: { $published } Started: { $started }
operator-status-node-live = Node { $node } { $state } caller={ $caller } lifecycle={ $lifecycle } transport={ $transport } duration={ $duration } file={ $file }
operator-status-node-presentation =   terminal={ $terminal } ansi={ $ansi } encoding={ $encoding } size={ $columns }x{ $rows } page={ $page } locale={ $locale } profile={ $profile } menu-mode={ $menu_mode } context={ $context } renderer={ $renderer } security={ $security } sysop-threshold={ $threshold } actions={ $actions }
operator-status-node-offline = Node { $node } { $state } { $description }
operator-console-title = SPITFIRE NG OPERATOR CONSOLE — { $board }
operator-console-commands = Commands: STATUS, PAGES, AVAILABLE ON|OFF, ANSWER <session>, DECLINE <session>, DISCONNECT <session>, CALLERS, IDENTITY, PROFILE, PROFILE-SET, ENABLE, DISABLE, DELETE, RESTORE, SECURITY, PURGE, SUBSCRIPTION, INFO-POLICY, INFO-POLICY-SET, BBS-LIST, BBS-ADD, BBS-EDIT, BBS-MOVE, BBS-STATE, QUIT
operator-console-prompt = Operator> { " " }
operator-console-availability-help = Use AVAILABLE ON or AVAILABLE OFF.
operator-console-chat-active = Chat active for session { $session }. /Q ends chat.
operator-console-unknown-command = Unknown operator command.
operator-console-caller-left = Caller left chat.
operator-console-caller-line = Caller> { $line }
operator-console-sysop-prompt = Sysop> { " " }
operator-console-node-row = Node { $node }: { $state } session={ $session } caller={ $caller } transport={ $transport }
operator-console-page-row = Session { $session } Node { $node } Caller { $caller }: { $state }
operator-console-caller-row = #{ $id } login={ $login } handle={ $handle } security={ $security } base={ $base_security } state={ $state } version={ $version } listed={ $listed } publicity-version={ $publicity_version } purge-protected={ $purge_protected } subscription={ $subscription } calls={ $calls }
operator-caller-lifecycle-active = Active
operator-caller-lifecycle-disabled = Locked Out
operator-caller-lifecycle-deleted = Deleted (recoverable)
operator-caller-protected = The configured named Sysop is protected from that operation.
operator-caller-conflict = That caller changed in another operator session. Reload the caller and try again.
operator-caller-security-changed = Caller base security changed.
operator-caller-disabled = Caller is now Locked Out.
operator-caller-enabled = Caller is active.
operator-caller-deleted = Caller is marked Deleted and remains recoverable.
operator-caller-restored = Caller identity was restored.
operator-caller-subscription-updated = Caller subscription was updated.
operator-caller-purge-updated = Caller purge protection was updated. No hard purge was performed.
operator-caller-identity-updated = Caller login identifier, display handle, and private real name were updated.
operator-console-profile-title = Private profile for { $caller }:
operator-console-profile-value = { $label }: { $value }
operator-console-profile-address-1 = Address 1
operator-console-profile-address-2 = Address 2
operator-console-profile-city = City
operator-console-profile-region = Region
operator-console-profile-postal = Postal Code
operator-console-profile-country = Country
operator-console-profile-phone = Phone
operator-console-profile-email = Email
operator-console-profile-birthday = Birth Date
operator-console-not-provided = [not provided]
operator-public-information-policy = Public directory enabled={ $enabled } last-call={ $last_call } location={ $location } caller-additions={ $caller_additions } version={ $version }
operator-public-information-policy-updated = Public-information policy updated; version={ $version }.
operator-public-information-conflict = Public information changed in another operator session. Reload and try again.
operator-other-bbs-row = #{ $id } order={ $order } state={ $state } version={ $version } contributor={ $contributor } { $name } | { $speed } | { $dial }
operator-other-bbs-updated = Other BBS entry #{ $id } updated; version={ $version }.

# Schema-18 operator observability. These labels describe privacy-bounded
# projections; they do not grant operator authority or define report output.
operator-activity-title = Board Activity
operator-activity-empty = No board activity matches the selected filters.
operator-activity-before-activation = Detailed activity is not available before operator observability was activated.
operator-activity-results = Showing { $count } board events.
operator-activity-more-results = More activity is available.
operator-event-time = Time
operator-event-node = Node
operator-event-category = Category
operator-event-severity = Severity
operator-event-outcome = Outcome
operator-event-details = Details
operator-event-filter-time = Time range
operator-event-filter-node = Node
operator-event-filter-category = Category
operator-event-filter-severity = Minimum severity
operator-event-filter-outcome = Outcome
operator-event-category-system = System
operator-event-category-node = Node
operator-event-category-session = Session
operator-event-category-caller = Caller
operator-event-category-authentication = Authentication
operator-event-category-message = Message
operator-event-category-file = File
operator-event-category-transfer = Transfer
operator-event-category-storage = Storage
operator-event-category-backup = Backup
operator-event-category-operator = Operator
operator-event-category-error = Error
operator-event-severity-info = Information
operator-event-severity-notice = Notice
operator-event-severity-warning = Warning
operator-event-severity-error = Error
operator-event-severity-critical = Critical
operator-event-outcome-succeeded = Succeeded
operator-event-outcome-failed = Failed
operator-event-outcome-cancelled = Cancelled
operator-event-outcome-denied = Denied
operator-event-outcome-unavailable = Unavailable
operator-event-outcome-observed = Observed
operator-statistics-title = System Statistics
operator-statistics-live-now = Live now
operator-statistics-today = Today
operator-statistics-lifetime = Lifetime
operator-statistics-calls-started = Calls started
operator-statistics-calls-completed = Calls completed
operator-statistics-call-history-note = Lifetime calls use the board's existing call counter. Detailed completion history begins when operator observability is activated.
operator-statistics-callers-online = Callers online
operator-statistics-active-nodes = Active nodes
operator-statistics-active-transfers = Active transfers
operator-statistics-messages = Messages posted
operator-statistics-uploads = Successful uploads
operator-statistics-downloads = Successful downloads
operator-statistics-upload-bytes = Bytes uploaded
operator-statistics-download-bytes = Bytes downloaded
operator-statistics-transfer-failures = Failed transfers
operator-statistics-transfer-cancellations = Cancelled transfers
operator-recent-callers-title = Recent Callers
operator-recent-callers-empty = No completed calls are available since operator observability was activated.
operator-notifications-title = Notifications
operator-notifications-empty = No operator notifications need attention.
operator-notification-open = Needs attention
operator-notification-acknowledged = Acknowledged
operator-notification-resolved = Resolved
operator-notification-acknowledgement-complete = Notification acknowledged.
operator-notification-conflict = That notification changed. Reload it and try again.
operator-notification-backup-failed = A board backup failed.
operator-notification-storage-unavailable = A configured storage location is unavailable.
operator-notification-node-fault = A node reported an operational fault.
operator-notification-operational-error = SPITFIRE NG reported an operational error.
operator-remediation-check-backup = Review the backup destination and board backup status, then retry safely.
operator-remediation-check-storage = Check the configured storage location and run a storage probe after it returns.
operator-remediation-check-node = Review the node state and recent related activity.
operator-remediation-review-event = Review the related activity entry for safe diagnostic details.
operator-maintenance-title = Maintenance Status
operator-maintenance-healthy = No maintenance condition currently needs attention.
operator-maintenance-open-notifications = Open notifications
operator-maintenance-recent-warnings = Warnings in the last 24 hours
operator-maintenance-recent-errors = Errors in the last 24 hours
operator-maintenance-storage-unavailable = Unavailable storage locations
operator-maintenance-pending-review = Files awaiting review
operator-maintenance-active-operations = Active or incomplete transfers
operator-retention-title = Activity Retention
operator-retention-detail = Detailed board activity is kept for { $days } days.
operator-retention-summary = Daily summaries are kept for { $days } days.
operator-retention-last-cleanup = Last cleanup: { $time }
operator-retention-not-run = Retention cleanup has not run yet.
operator-retention-conflict = Retention settings changed elsewhere. Reload them before trying again.
