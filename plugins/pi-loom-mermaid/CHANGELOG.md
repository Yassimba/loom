# Changelog

## [0.3.0](https://github.com/Yassimba/loom/compare/pi-loom-mermaid-v0.2.0...pi-loom-mermaid-v0.3.0) (2026-09-06)


### Features

* improve setup and Mermaid rendering ([#188](https://github.com/Yassimba/loom/issues/188)) ([f77b4ac](https://github.com/Yassimba/loom/commit/f77b4ac5c42e406a1f7eb2eb21759a390478719b))

## [0.2.0](https://github.com/Yassimba/loom/compare/pi-loom-mermaid-v0.1.0...pi-loom-mermaid-v0.2.0) (2026-09-06)


### Features

* add colored Mermaid rendering for Pi ([6d50539](https://github.com/Yassimba/loom/commit/6d50539844307f6af8a19aecb4436e4440e5d773))
* add colored Mermaid rendering for Pi ([57814fe](https://github.com/Yassimba/loom/commit/57814fe3e68c88e1eb6d3d20bd156a648ef9424b))
* **loom:** add Sem MCP and colored Mermaid rendering ([db54b81](https://github.com/Yassimba/loom/commit/db54b819bee319fbda6f22b450396a4c4f24e1bc))
* **pi-loom-mermaid:** a head per arrival where the top has room ([a66453f](https://github.com/Yassimba/loom/commit/a66453f489bc2be1c4f860e9355a6483a2e93c64))
* **pi-loom-mermaid:** Brandes–Köpf placement and interior skip routes ([a104ca0](https://github.com/Yassimba/loom/commit/a104ca022723342d72b7bfea28dd2adb773929b1))
* **pi-loom-mermaid:** cap the width a straighter placement run may cost ([0a9d2e7](https://github.com/Yassimba/loom/commit/0a9d2e7f863d558d957ee312b6a850ef6fbb0dfe))
* **pi-loom-mermaid:** colored rendering with layered layout and interior routing ([21e71fb](https://github.com/Yassimba/loom/commit/21e71fbf4384f0ff7e30b2af8fe6f9430612bf0d))
* **pi-loom-mermaid:** concentrate parallel chains into trunks ([f06c8aa](https://github.com/Yassimba/loom/commit/f06c8aad412ce1f3e1d88040d04c987077c451ed))
* **pi-loom-mermaid:** draw true crossings as hops ([4a2dd15](https://github.com/Yassimba/loom/commit/4a2dd158b2a1232c4a868e5f6dcd9a5f1c06d769))
* **pi-loom-mermaid:** label long edges beside their chain ([ca7acbc](https://github.com/Yassimba/loom/commit/ca7acbcd2c30d0b567b9bd1ac359e47bd636f2c7))
* **pi-loom-mermaid:** let a skip enter on its chain column ([f3cb1de](https://github.com/Yassimba/loom/commit/f3cb1dee781771597a19d704a1a460ae4248dba9))
* **pi-loom-mermaid:** normalise long edges and transpose during crossing reduction ([34676c9](https://github.com/Yassimba/loom/commit/34676c92c80bb7422b8ad7c90105ae74719179c8))
* **pi-loom-mermaid:** order bus tracks by crossings, not packing ([2b7062d](https://github.com/Yassimba/loom/commit/2b7062d52742c148a011584e8639adafdd3305b3))
* **pi-loom-mermaid:** pick a back edge's port side by what its arm would cross ([590bcf4](https://github.com/Yassimba/loom/commit/590bcf42e49bb53ddc2a1fc327e6b6dfc1050cc8))
* **pi-loom-mermaid:** relayout with tighter labels when wider than the space ([137f94b](https://github.com/Yassimba/loom/commit/137f94b83c1db25353f99dcc96462ae1ca626c57))
* **pi-loom-mermaid:** route top-down back edges through the interior ([ad1c6d7](https://github.com/Yassimba/loom/commit/ad1c6d70242ed0694a04baa8f7aed7d06958002f))
* **pi-loom-mermaid:** shorten long edges with node promotion ([b414269](https://github.com/Yassimba/loom/commit/b4142691c2623cf18b86c78a363476a873a2d4be))
* **pi-loom-mermaid:** weighted-median sweeps with seeded restarts ([942aa5a](https://github.com/Yassimba/loom/commit/942aa5a6be0bded39302cd6f728c48907c18054b))


### Bug Fixes

* **pi-loom-mermaid:** draw edges collapsed onto one frame once ([82f36ad](https://github.com/Yassimba/loom/commit/82f36ad86b96e47697803675dad3959f5f9b6fa4))
* **pi-loom-mermaid:** keep a cell between arrowheads ([469af9a](https://github.com/Yassimba/loom/commit/469af9ae3de8df1a16f7c118a26e15c59d40153d))
* **pi-loom-mermaid:** keep lane endpoints trailing inside crossing reduction ([08aadc5](https://github.com/Yassimba/loom/commit/08aadc5d2373269587b17b61fc24d62f69b4b0ea))
* **pi-loom-mermaid:** keep left-to-right skips on the lane ([d4501a2](https://github.com/Yassimba/loom/commit/d4501a27d463c74dd4e55669881c497005f30dc5))


### Performance Improvements

* **pi-loom-mermaid:** cache rendered blocks; keep unclosed fences as source while streaming ([74dac69](https://github.com/Yassimba/loom/commit/74dac69369c91691e00886860528486eed2ecb75))
* **pi-loom-mermaid:** trim rows without a backtracking regex ([88a315f](https://github.com/Yassimba/loom/commit/88a315f63eb14bb0fa5f5138fada1df6c2ebf566))
