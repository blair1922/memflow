#!/bin/bash
rustup run nightly cglue-bindgen -- --config cbindgen.toml --crate memflow-ffi --output memflow.h -l C
rustup run nightly cglue-bindgen -- --config cbindgen.toml --crate memflow-ffi --output memflow.hpp -l C++
