# Changelog

This file documents recent notable changes to this project. The format of this
file is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and
this project adheres to [Semantic
Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.0] - 2021-06-10

### Changed

- `pcap::Input` no longer requires a type parameter.
- Turned off default features of kafka.

## [0.5.1] - 2021-01-19

### Fixed

- Require `Send` explicitly for `Input` to avoid possible data racing 

## [0.5.0] - 2020-11-02

### Changed

- Requires Rust 1.44 or higher.
- Updated nom to 6.

## [0.4.0] - 2020-10-12

### Changed

- Requires Rust 1.38 or higher.
- Updated pcap-parser to 0.9 and crossbeam-chaneel to 0.5.

## [0.3.5] - 2020-02-03

- Updated documentation.

## [0.3.4] - 2019-12-30

### Changed

- Requires pcap-parser>=0.8.1. pcap-parser-0.8.0 fails to compile with
  cookie-factory-0.3.0.

## [0.3.3] - 2019-11-08

### Added

- `mbox::Input` reads each email in a mbox file as an event.

## [0.3.2] - 2019-11-07

### Changed

- `pcap::Input` became `Send`.

## [0.3.1] - 2019-11-05

### Added

- `split` spawns multiple threads to process events in parallel.

## [0.3.0] - 2019-11-01

### Added

- Traits to handle events, common to all the input types.

### Changed

- `text::Input` can handle a non-UTF-8 text input.

## [0.2.0] - 2019-10-30

### Added

- `text::Input` reads each line in a text file as an event.
- `pcap::Input` reads each packet in a pcap file as an event.

### Changed

- `kafka::Input` fetches no more entries than the specified limit.

## [0.1.0] - 2019-10-14

### Added

- Kafka input/output and an example of their usage.

[0.6.0]: https://github.com/petabi/eventio/compare/0.5.1...0.6.0
[0.5.1]: https://github.com/petabi/eventio/compare/0.5.0...0.5.1
[0.5.0]: https://github.com/petabi/eventio/compare/0.4.0...0.5.0
[0.4.0]: https://github.com/petabi/eventio/compare/0.3.5...0.4.0
[0.3.5]: https://github.com/petabi/eventio/compare/0.3.4...0.3.5
[0.3.4]: https://github.com/petabi/eventio/compare/0.3.3...0.3.4
[0.3.3]: https://github.com/petabi/eventio/compare/0.3.2...0.3.3
[0.3.2]: https://github.com/petabi/eventio/compare/0.3.1...0.3.2
[0.3.1]: https://github.com/petabi/eventio/compare/0.3.0...0.3.1
[0.3.0]: https://github.com/petabi/eventio/compare/0.2.0...0.3.0
[0.2.0]: https://github.com/petabi/eventio/compare/0.1.0...0.2.0
[0.1.0]: https://github.com/petabi/eventio/tree/0.1.0
