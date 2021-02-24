# eventio

eventio is a collection of event I/O processors for event-processing
applications. It allows to collect events from various sources, distribute them
to one or more event processors, and optionally collect the processing results
to send them to storage or other event processors.

[![crates.io](https://img.shields.io/crates/v/eventio)](https://crates.io/crates/eventio)
[![Documentation](https://docs.rs/eventio/badge.svg)](https://docs.rs/eventio)
[![Coverage Status](https://codecov.io/gh/petabi/eventio/branch/master/graphs/badge.svg)](https://codecov.io/gh/petabi/eventio)

## Requirements

* Rust ≥ 1.44
* OpenSSL ≥ 1.0.1 or LibreSSL ≥ 2.5

## License

Copyright 2019-2021 Petabi, Inc.

Licensed under [Apache License, Version 2.0][apache-license] (the "License");
you may not use this crate except in compliance with the License.

Unless required by applicable law or agreed to in writing, software distributed
under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
CONDITIONS OF ANY KIND, either express or implied. See [LICENSE](LICENSE) for
the specific language governing permissions and limitations under the License.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the [Apache-2.0
license][apache-license], shall be licensed as above, without any additional
terms or conditions.

[apache-license]: http://www.apache.org/licenses/LICENSE-2.0
