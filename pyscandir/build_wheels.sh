#!/usr/bin/env bash
eval "$(pyenv init -)"

pyenv shell 3.7.9 3.8.10 3.9.12 3.10.4 3.11.0a7
maturin build --release --strip --no-sdist
