![build status](https://travis-ci.com/BourgondAries/fast-logger.svg?branch=master "build status")
[![Latest version](https://img.shields.io/crates/v/fast-logger.svg)](https://crates.io/crates/fast-logger)
[![Documentation](https://docs.rs/fast-logger/badge.svg)](https://docs.rs/fast-logger)

# Fast logger #

Fast-logger is a logger for Rust that attempts to be the fastest (with the lowest caller-latency time) logger for rust. It achieves this by not performing dynamic allocations and passing formatting data over a channel to another thread. The reason for doing this is that formatting itself is an expensive operation.
