# Native Windows

`loom setup` prints the authoritative list of capabilities that require WSL2. Report every item; do not maintain another list.

If the user wants the complete setup, let Loom reuse an existing WSL2 distribution or offer Ubuntu installation. Explain that installation may require elevation and a reboot before confirmation. Afterward, follow Loom's printed command to open the distribution and run the Unix bootstrap inside it.

If the user stays native, continue with the resources Loom offers and report every omitted WSL-only capability. An explicit request for an omitted capability must move to WSL.
