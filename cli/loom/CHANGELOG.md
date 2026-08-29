# Changelog

## [0.14.0](https://github.com/Yassimba/loom/compare/loom-v0.13.2...loom-v0.14.0) (2026-08-29)


### Features

* **loom:** show which update lanes are still running ([1633482](https://github.com/Yassimba/loom/commit/163348213d38cb6ae3b0c08c7d1106a04c05bc5a))


### Bug Fixes

* **loom:** install Pi from its new npm scope without mise ([964bc70](https://github.com/Yassimba/loom/commit/964bc7021441718093a00dde42faa6bdca04d186))
* **manifest:** pin Pi under its new npm scope so extensions load again ([ab7e9b9](https://github.com/Yassimba/loom/commit/ab7e9b95ada532cca6a446852f8c68ccdbbb1eb5))

## [0.13.2](https://github.com/Yassimba/loom/compare/loom-v0.13.1...loom-v0.13.2) (2026-08-29)


### Bug Fixes

* **manifest:** give loom-teams its own mise backend so it stops evicting loom ([dd57a0c](https://github.com/Yassimba/loom/commit/dd57a0cb43e11cabe59159d2846418bfcef18243))

## [0.13.1](https://github.com/Yassimba/loom/compare/loom-v0.13.0...loom-v0.13.1) (2026-08-28)


### Bug Fixes

* **loom:** survive a piped install and stop copying tool pins into skill trees ([946ce05](https://github.com/Yassimba/loom/commit/946ce05aeebb5c15dcbd774495d47112c303c2e7))

## [0.13.0](https://github.com/Yassimba/loom/compare/loom-v0.12.0...loom-v0.13.0) (2026-08-28)


### Features

* **changeset-walkthrough:** add the figure-led git-change walkthrough skill ([4c65506](https://github.com/Yassimba/loom/commit/4c655060e3d91b707427eb8ec08848ad37460fe6))
* **loom:** rank wizard search with nucleo-matcher ([5076a26](https://github.com/Yassimba/loom/commit/5076a2662f7ebeb02178f7afa2c46bf98501b9a2))
* **loom:** redesign the setup wizard and unify report output ([511475b](https://github.com/Yassimba/loom/commit/511475b3541a1ad4c700269575c8e86c1fd6a986))
* **loom:** redesign the setup wizard and unify report output ([6741dcf](https://github.com/Yassimba/loom/commit/6741dcfd0ca3a9e42b4817e4cc25d3a751a20117))
* **loom:** replace bundles with an Everything group ([de92c94](https://github.com/Yassimba/loom/commit/de92c94c0482c262d0379f17b43f4f9e04922447))
* **loom:** three-column Choose step and a polish pass ([c1274d2](https://github.com/Yassimba/loom/commit/c1274d259e79265ed4eef1138697d29bbee99504))
* **manifest:** replace claude-bridge with anthropic-auth ([dfe253b](https://github.com/Yassimba/loom/commit/dfe253b690f2132118a675f248ff66808bc9de3b))
* **manifest:** replace claude-bridge with anthropic-auth ([23320b7](https://github.com/Yassimba/loom/commit/23320b72e7fea53563ec6b2874a73e201b1617a6))
* **setup:** refresh curated packages and skills ([47a134e](https://github.com/Yassimba/loom/commit/47a134e01b0578fea91eea9398be7be264386464))
* **skills:** add frontend-slides ([e65edf0](https://github.com/Yassimba/loom/commit/e65edf0390b102b25c49803270aa4a0621f2d68d))


### Bug Fixes

* **catalog:** regenerate curated setup catalog ([d61ee8f](https://github.com/Yassimba/loom/commit/d61ee8f0513833f851abbb5d65ab5b88a8c38335))
* **loom:** survive narrow terminals and size columns from content ([488e61a](https://github.com/Yassimba/loom/commit/488e61a7137b1525e6a1cfcdeb40cebd6efcfa10))

## [0.12.0](https://github.com/Yassimba/loom/compare/loom-v0.11.0...loom-v0.12.0) (2026-08-28)


### Features

* loom-teams CLI, release-please pipeline, catalog reshape ([548af3c](https://github.com/Yassimba/loom/commit/548af3c61a7ddd01aeb8d8454179cc9abbaed22f))
* next version of my skills slowly going to consolidate on a single approach for reviews ([ccf55dc](https://github.com/Yassimba/loom/commit/ccf55dc786c366888e71ab8bc6cc7733f9c0481e))
* **skills:** reshape the catalog around research, codegraph, and annotate ([40a7502](https://github.com/Yassimba/loom/commit/40a7502fa521507d94b96be8d99080a313970807))
* **skills:** reshape the skill catalog and fork plannotator ([687c820](https://github.com/Yassimba/loom/commit/687c820b9e1280b0e924de105d6197013fd5a6df))


### Bug Fixes

* **loom:** satisfy clippy manual_slice_fill in preset reset ([17a89e2](https://github.com/Yassimba/loom/commit/17a89e2473ea1a2161fdef709ccedaae335b114a))

## [0.11.0] - 2026-08-15

## [0.10.1] - 2026-08-08

## [0.10.0] - 2026-08-08

## [0.9.1] - 2026-08-08

## [0.9.0] - 2026-08-08

## [0.8.0] - 2026-08-08

## [0.7.0] - 2026-08-08

## [0.6.2] - 2026-07-16

## [0.6.1] - 2026-07-16

## [0.6.0] - 2026-07-16

## [0.5.0] - 2026-07-13

## [0.4.0] - 2026-07-12

## [0.3.2] - 2026-07-12

## [0.3.1] - 2026-07-12

## [0.3.0] - 2026-07-12

## [0.2.0] - 2026-07-12

- feat(reviewr): Deep Review collaboration and Pi agent following (#11) (`9270914`)
- feat: ratatui setup wizard, reviewr thread loading, subagent fixes (#10) (`ce86351`)
- fix: ton of fixes for better CI/CD and other cool stuff to improve my AI setup (`876042e`)
- feat(pi-subagents): add external advisor workflows (`730fd1d`)
