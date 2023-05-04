main.o: target/release/libhelloasm.rlib
	f=$$(ar t target/release/libhelloasm.rlib | grep -vxF lib.rmeta) && \
	  ar x target/release/libhelloasm.rlib "$$f" && \
	  mv "$$f" main.o

target/release/libhelloasm.rlib: src/lib.rs
	cargo build --release
