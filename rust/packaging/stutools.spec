%global debug_package %{nil}

Name:           stutools
Version:        2.0.0
Release:        1%{?dist}
Summary:        Stu's I/O tester, Rust implementation
License:        Apache-2.0 AND MIT
URL:            https://github.com/dr-who/stutools
Source0:        %{name}-%{version}.tar.gz
# Upstream release builds use the locked, offline vendor bundle. Keep these
# security-tracking Provides in sync with Cargo.lock, including in prebuilt RPMs.
Provides:       bundled(crate(bitflags)) = 2.13.2
Provides:       bundled(crate(cfg-if)) = 1.0.5
Provides:       bundled(crate(io-uring)) = 0.7.15
Provides:       bundled(crate(libc)) = 0.2.191
Provides:       bundled(crate(pin-project-lite)) = 0.2.17
Provides:       bundled(crate(tokio)) = 1.54.0
%if 0%{?prebuilt}
Source1:        spit
%else
BuildRequires:  rust >= 1.85
BuildRequires:  cargo >= 1.85
BuildRequires:  gcc
BuildRequires:  libaio-devel
BuildRequires:  python3
%endif

%description
A fixed-pool I/O benchmark with libaio, io_uring, direct and buffered backends.
This Rust migration currently supplies spit; use the C tree for the other tools.

%prep
%setup -q

%build
%if !0%{?prebuilt}
cargo build --release --locked --offline
%endif

%check
%if !0%{?prebuilt}
cargo test --locked --offline
%endif

%install
install -d %{buildroot}%{_bindir}
install -d %{buildroot}%{_mandir}/man1
%if 0%{?prebuilt}
install -m755 %{SOURCE1} %{buildroot}%{_bindir}/spit
install -m644 spit.1 %{buildroot}%{_mandir}/man1/spit.1
%else
install -m755 target/release/spit %{buildroot}%{_bindir}/spit
install -m644 packaging/spit.1 %{buildroot}%{_mandir}/man1/spit.1
%endif

%files
%license LICENSE THIRD_PARTY_LICENSES.txt
%doc README.md INSTALL.md CCTV.md
%{_bindir}/spit
%{_mandir}/man1/spit.1*

%changelog
* Wed Oct 21 2026 Stuart Inglis <stuart.inglis@gmail.com> - 2.0.0-1
- Add the Rust spit implementation and offline source packaging.
