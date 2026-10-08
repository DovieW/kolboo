# Windows setup upgrade policy

`installer.nsi` is Tauri CLI **2.11.4**'s stock NSIS template, licensed under
MIT OR Apache-2.0 (the MIT license is included here). The `KOLBOO` block
immediately after version comparison in `PageReinstall` sets `UpdateMode` for
newer NSIS versions and skips the uninstall/maintenance page. `SkipIfUpdating`
also skips directory and shortcut-folder selection to retain the original
installation location and shortcuts, rather than accidentally creating a
second installation.
The stock copy/install, process-running checks, directory restoration,
signature handling, shortcuts and uninstall/data-deletion safeguards remain.

First installs, same-version repair, downgrade handling and MSI-to-NSIS
migration keep their stock behavior. Do not bypass MSI migration by overwriting
its files: that leaves stale Windows Installer ownership. Automatic updates
publish both `windows-x86_64-nsis` and `windows-x86_64-msi`, so an installed MSI
receives its own major-upgrade package instead of migrating to NSIS.

When upgrading `@tauri-apps/cli`, rebase this template against the corresponding
upstream template and run the Windows packaging/upgrade rehearsal. Do not copy
an unrelated `dev` template. Installer contract tests verify the pinned CLI,
template provenance and the unchanged updater public key.

Windows acceptance must exercise: fresh EXE install; newer EXE over EXE without
the uninstall page; same-version repair; MSI over MSI; background signed update
followed by Quit; tampered download; offline launch; and preservation of saved
keys, recordings, settings, startup choice and user-deleted shortcuts. MSI's
normal major-upgrade replacement is performed by Windows Installer, not a
separate uninstall task for the user. Machine-wide MSI installs can still need
Windows administrator approval. The signed updater's normal installer handoff
restarts the updated application.

Implementation validation on October 7, 2026 covers the pinned-template and
manifest contracts, real signed downloads against an isolated local server,
and installation/quit lifecycle on an isolated Linux desktop. This is not
Windows package acceptance: the Windows cases above still need rehearsal
before shipping this installer change.
