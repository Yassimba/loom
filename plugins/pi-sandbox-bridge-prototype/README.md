# Pi sandbox bridge prototype

A thin wrapper around `pi-sandbox` 0.6.5. It runs the upstream extension unchanged and adds an internal `pi-sandbox:grant-path` event for trusted local extensions.

The event accepts a read/write permission and a session/project/global scope. It routes the request through the upstream `/sandbox-allow` handler without displaying its permission prompt. This lets `/add-dir` show one combined access-and-lifetime prompt while `pi-sandbox` remains the permission owner.

Install this package instead of loading `pi-sandbox` separately. Loading both registers the sandbox twice.
