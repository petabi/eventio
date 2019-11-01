# Changelog

This file documents all notable changes to this project. The format of this file
is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/petabi/eventio/compare/0.2.0...HEAD
[0.2.0]: https://github.com/petabi/eventio/compare/0.1.0...0.2.0
[0.1.0]: https://github.com/petabi/eventio/tree/0.1.0
