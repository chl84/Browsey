Name:           browsey
Version:        0.4.4
Release:        1%{?dist}
Summary:        Minimalist and fast file explorer built with Tauri

License:        MIT
URL:            https://github.com/chl84/Browsey
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  nodejs
BuildRequires:  npm
BuildRequires:  pkgconfig(gtk+-3.0)
BuildRequires:  pkgconfig(javascriptcoregtk-4.1)
BuildRequires:  pkgconfig(libsoup-3.0)
BuildRequires:  pkgconfig(webkit2gtk-4.1)

Requires:       gtk3
Requires:       webkit2gtk4.1

%description
Browsey is a minimalist and fast cross-platform file explorer built with a
Rust/Tauri backend and a Svelte frontend.

%prep
%setup -q -c -T
# Extract source regardless of archive top-level directory name.
tar -xf %{SOURCE0} --strip-components=1

%build
npm --prefix frontend ci
npm --prefix frontend run build
cargo build --release --locked

%install
install -d %{buildroot}%{_bindir}
install -m 0755 target/release/browsey %{buildroot}%{_bindir}/browsey
install -m 0755 resources/pdfium-linux-x64/lib/libpdfium.so %{buildroot}%{_bindir}/libpdfium.so

install -d %{buildroot}%{_libdir}/Browsey
install -m 0644 THIRD_PARTY_NOTICES %{buildroot}%{_libdir}/Browsey/THIRD_PARTY_NOTICES

install -d %{buildroot}%{_datadir}/applications
cat > %{buildroot}%{_datadir}/applications/Browsey.desktop <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Browsey
Exec=browsey
Icon=browsey
Terminal=false
StartupWMClass=browsey
Categories=System;FileTools;FileManager;
DESKTOP

install -d %{buildroot}%{_datadir}/icons/hicolor/512x512/apps
install -m 0644 resources/icons/icon.png %{buildroot}%{_datadir}/icons/hicolor/512x512/apps/browsey.png

%files
%license LICENSE
%doc README.md
%{_bindir}/browsey
%{_bindir}/libpdfium.so
%{_libdir}/Browsey/THIRD_PARTY_NOTICES
%{_datadir}/applications/Browsey.desktop
%{_datadir}/icons/hicolor/512x512/apps/browsey.png

%changelog
* Fri Feb 20 2026 Chris <chris@example.com> - 0.4.4-1
- Initial COPR spec for Browsey
