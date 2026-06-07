FROM rust:1-bookworm

# delta-api's build.rs builds the native libasp during `cargo build` (the
# ASP_NATIVE_BUILD path used in CI): git clones the pinned asp, cmake + ninja +
# a compiler build it, and libclang lets bindgen generate the FFI bindings.
# pkg-config / libssl-dev cover native deps in the crate graph.
RUN apt-get update && apt-get install --no-install-recommends -y \
	git \
	cmake \
	ninja-build \
	clang \
	libclang-dev \
	build-essential \
	pkg-config \
	libssl-dev \
	&& rm -rf /var/lib/apt/lists/*
