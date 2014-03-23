RUSTC = rustc
RUSTDOC = rustdoc
RUSTFLAGS = -O
BUILDDIR = build
TESTDIR = $(BUILDDIR)/test

all: $(BUILDDIR) test lib docs

$(BUILDDIR):
	mkdir -p $@

$(TESTDIR): $(BUILDDIR)
	mkdir -p $@

lib:
	$(RUSTC) $(RUSTFLAGS) --out-dir $(BUILDDIR) src/lib.rs

clean:
	rm -rf $(BUILDDIR)

test: libtest doctest

libtest: $(TESTDIR)
	$(RUSTC) --test -o $(TESTDIR)/test src/lib.rs
	RUST_LOG=std::rt::backtrace ./$(TESTDIR)/test

doctest: lib
	$(RUSTDOC) -L $(BUILDDIR) --test src/lib.rs

docs:
	$(RUSTDOC) src/lib.rs
