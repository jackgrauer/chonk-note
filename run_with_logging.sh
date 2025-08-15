#!/bin/bash
# Run chonker7 with stderr redirected to a log file
DYLD_LIBRARY_PATH=./lib ./target/release/chonker7 "$@" 2>chonker_tables.log
