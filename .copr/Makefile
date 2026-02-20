SHELL := /bin/bash

.PHONY: srpm
srpm:
	set -euo pipefail; \
	spec_path="$${spec:-browsey.spec}"; \
	out_dir="$${outdir:-.}"; \
	name="$$(rpmspec --srpm --queryformat '%{NAME}\n' -q "$$spec_path" | head -n1)"; \
	version="$$(rpmspec --srpm --queryformat '%{VERSION}\n' -q "$$spec_path" | head -n1)"; \
	topdir="$$(mktemp -d)"; \
	trap 'rm -rf "$$topdir"' EXIT; \
	mkdir -p "$$topdir"/BUILD "$$topdir"/BUILDROOT "$$topdir"/RPMS "$$topdir"/SOURCES "$$topdir"/SPECS "$$topdir"/SRPMS; \
	git archive --format=tar.gz --prefix="$$name-$$version/" -o "$$topdir/SOURCES/$$name-$$version.tar.gz" HEAD; \
	cp "$$spec_path" "$$topdir/SPECS/"; \
	rpmbuild -bs --define "_topdir $$topdir" "$$topdir/SPECS/$$(basename "$$spec_path")"; \
	mkdir -p "$$out_dir"; \
	cp -v "$$topdir"/SRPMS/*.src.rpm "$$out_dir"/
