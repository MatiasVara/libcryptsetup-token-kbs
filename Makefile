TARGET      = libcryptsetup-token-tdx-kbs.so
INSTALL_DIR = /usr/lib64/cryptsetup
STATIC_LIB  = target/release/liblibcryptsetup_token_tdx_kbs.a
VERSION_MAP = src/libcryptsetup-token-tdx-kbs.sym

.PHONY: all clean install

all: $(TARGET)

$(STATIC_LIB): src/*.rs Cargo.toml
	cargo build --release

$(TARGET): $(STATIC_LIB) $(VERSION_MAP)
	gcc -shared -o $@ \
		-Wl,--whole-archive $(STATIC_LIB) -Wl,--no-whole-archive \
		-Wl,--version-script=$(VERSION_MAP) \
		-lcryptsetup -lpthread -ldl -lm -lrt \
		-static-libgcc

install: $(TARGET)
	install -D -m 755 $(TARGET) $(DESTDIR)$(INSTALL_DIR)/$(TARGET)

clean:
	cargo clean
	rm -f $(TARGET)
