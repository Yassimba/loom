# @yassimba/pi-add-dir

Add another directory to Pi's system prompt. When you add it, the active model creates a one-sentence project orientation from its root `README.md` and `package.json`. The next agent turn sees the path, the orientation, and the names of available root instruction files. It does not receive the instruction contents. The agent must read those files before it operates in the directory. The working directory does not change.

Choose whether each directory applies to this session, this project, or all projects. Global directories require confirmation because their orientation appears in every project. Added directories show as `added dirs …` in Pi's footer.

## Install

From Loom:

```bash
loom add --pi-package add-dir
```

Or in Pi:

```bash
pi install npm:@yassimba/pi-add-dir
```

From this repo without installing:

```bash
pi -e ./plugins/pi-add-dir
```

## Use

```text
/add-dir ../other-repo
/rm-dir other-repo
/dirs
```

`/add-dir` completes directories; Tab opens the highlighted child. zoxide-frequent paths sort first when zoxide is installed. Project choices are stored in `.pi/add-dir.json`; global choices are stored in Pi's agent directory.
