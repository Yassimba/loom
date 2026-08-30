# @yassimba/pi-add-dir

Add another directory to Pi's system prompt. The next agent turn sees each path, plus root `AGENTS.md` and `CLAUDE.md` if they exist. The working directory does not change.

Choose whether each directory applies to this session, this project, or all projects. Global directories require confirmation because their instructions affect every project. Added directories show as `dirs …` in Pi's footer.

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
