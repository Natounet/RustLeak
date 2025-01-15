TARGETS = x86_64-unknown-linux-gnu x86_64-pc-windows-gnu
.PHONY: all $(TARGETS)

all: $(TARGETS)

$(TARGETS):
	cross build --target $@ --release && strip target/$@/release/$(shell basename $(CURDIR))
