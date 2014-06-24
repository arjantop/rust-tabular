RUSTC = rustc
RUSTDOC = rustdoc
RUSTFLAGS = -O
BUILDDIR = target
TESTDIR = $(BUILDDIR)/test
EXAMPLEDIR = $(BUILDDIR)/examples

all: $(BUILDDIR) test lib docs examples

$(BUILDDIR):
	mkdir -p $@

$(TESTDIR): $(BUILDDIR)
	mkdir -p $@

$(EXAMPLEDIR): $(BUILDDIR)
	mkdir -p $@

lib:
	cargo build

clean:
	rm -rf $(BUILDDIR)

test: libtest doctest

libtest: $(TESTDIR)
	$(RUSTC) --test -o $(TESTDIR)/test src/tabular.rs
	RUST_LOG=std::rt::backtrace ./$(TESTDIR)/test

doctest: lib
	$(RUSTDOC) -L $(BUILDDIR) --test src/tabular.rs

bench: $(TESTDIR)
	$(RUSTC) $(RUSTFLAGS) --test -o $(TESTDIR)/bench src/tabular.rs
	./$(TESTDIR)/bench --bench

docs:
	$(RUSTDOC) src/tabular.rs

examples: lib $(EXAMPLEDIR)
	$(RUSTC) $(RUSTFLAGS) -L $(BUILDDIR) -o $(EXAMPLEDIR)/read_csv examples/read_csv.rs
	$(RUSTC) $(RUSTFLAGS) -L $(BUILDDIR) -o $(EXAMPLEDIR)/read_tsv examples/read_tsv.rs
