# Input host settings contract (Input 0.18.0)

The exact Input 0.18.0 main bundle owns a three-boolean application-settings
DTO: `showedAnalyticsPopUp`, `analyticsConsented`, and
`smartActionCmdEnabled`. Renderer IPC reads the complete DTO through
`common-get-app-settings` and writes it through `common-send-app-settings`.
The main process converts that DTO to its application-settings model and saves
it through `ApplicationService`; the LokiJS `app_settings` collection in
`input_storage.json` is an implementation detail, not a CLI write target.

The command Smart Action editor toggles only `smartActionCmdEnabled` on the
complete in-memory DTO. The `kb.sa.exec` host notification checks the same
field immediately before executing a command and returns without execution
when it is false. The observed default is `false`.

## CLI contract

WorkLouderCTL exposes the field through the versioned Input Companion Bridge.
Snapshots contain all three booleans plus a canonical revision. A command
permission candidate changes only `smartActionCmdEnabled`, preserving both
analytics fields. Apply and restore require compare-and-swap, idempotency,
immutable backup, complete DTO replacement through Input, exact readback, and
automatic rollback. Direct `input_storage.json` mutation stays outside this
adapter.
