Synchronizing the Maestro fork
==============================

The fork retains upstream history and adds Maestro support. Keeping the changes
to shared files small reduces merge conflicts; tests are still needed to detect
changes in upstream behavior that merge without a textual conflict.

Keep provider code separate
----------------------------

Put Maestro implementation and tests in Maestro-owned files. The C payload
adapter lives in ``src/cext/maestro.rs`` and the Lua adapter in
``lua/maestro_local.c``. Shared dispatch code should only connect those adapters
to QRMI. Keep the public payload layouts and resource enum values compatible
with upstream; an enum insertion still needs explicit ABI review and rebuilding
the affected consumers.

Preserve the surrounding upstream layout when changing shared files. In
particular, avoid whole-file formatting of ``lua/lua_qrmi.c`` or JSON schemas.
Format new C adapter files with LLVM style and four-space indentation. Continue
to run Rustfmt and the pinned Black and Pylint versions as described in
:doc:`CONTRIBUTING`.

Merge without losing ancestry
-----------------------------

Sync after upstream releases or changes to shared interfaces, while each update
is small. Start from a clean checkout and refresh both remotes:

.. code-block:: sh

   git fetch origin main
   git fetch upstream main
   git worktree add -b sync/upstream-next ../qrmi-upstream-next origin/main
   cd ../qrmi-upstream-next
   git merge --no-commit --no-ff upstream/main

Resolve conflicts and run the checks below before committing. Retain that merge
commit when integrating into ``main``: use a fast-forward or a normal merge,
rather than squash-merging or cherry-picking the sync. Losing the upstream
ancestry makes later synchronization revisit changes that are already present.

Git can remember repeated resolutions. Enable this separately in each clone:

.. code-block:: sh

   git config --local rerere.enabled true
   git config --local rerere.autoupdate false

Review reused resolutions before staging them. This cache helps with recurring
conflicts; it does not make new interface changes compatible automatically and
is not transferred to other clones or GitHub's Sync fork button.

Lockfiles and validation
-------------------------

Preserve upstream dependency versions and the Maestro workspace entries when
resolving lockfiles. Let Cargo validate the result; do not run an unrestricted
``cargo update`` as part of every sync. If an adjustment is necessary, update
the specific package and verify it with the pinned toolchain. The root workspace
and ``examples/qrmi/rust`` have separate lockfiles and both need checking.

Run the checks in :doc:`CONTRIBUTING`, including strict Clippy for both Rust
workspaces, the task runner and Python bindings. Rebuild the Python extension
before its unit and compatibility tests, run the configured pre-commit hook,
and build Sphinx with warnings treated as errors. The combined Python coverage
must meet the threshold in ``.github/workflows/testing.yml``.

When the bindings change, rebuild the C library and Lua module with the matching
generated ``qrmi.h``. The scripted Lua/C regressions need no live Maestro daemon:

.. code-block:: sh

   QRMI_TEST_LIBRARY="$PWD/target/debug/libqrmi.so" \
   QRMI_TEST_LUA_MODULE="$PWD/target/lua/qrmi.so" \
   QRMI_TEST_LUA=/usr/bin/lua5.3 \
       python -m pytest python/tests/unit/quantum_resource/test_maestro_local.py

Use the Lua interpreter matching the module's build. These checks exercise
payload defaults, explicit options, error handling and detached sessions.

Documentation publication
-------------------------

The Sphinx workflow builds documentation on forks. Publication to GitHub Pages
is enabled automatically for upstream. To publish a fork's documentation,
enable GitHub Pages with GitHub Actions as the source, then set the repository
Actions variable ``QRMI_DEPLOY_PAGES`` to ``true``. Without that opt-in, the
deployment job is skipped and the build checks still run.
