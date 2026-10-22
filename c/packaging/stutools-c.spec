Name:           stutools-c
Version:        1.1.3
Release:        %{?package_release}%{!?package_release:1}%{?dist}
Summary:        Stu's storage and network I/O testing tools
License:        Apache-2.0 AND MIT
URL:            https://github.com/dr-who/stutools
Source0:        stutools-%{version}.tar.gz
Conflicts:      stutools < 2.0.0
BuildRequires:  gcc
BuildRequires:  gcc-c++
BuildRequires:  make
BuildRequires:  cmake >= 3.20
BuildRequires:  libaio-devel
BuildRequires:  pciutils-devel
BuildRequires:  numactl-devel
BuildRequires:  readline-devel
BuildRequires:  libuuid-devel
BuildRequires:  python3
BuildRequires:  git

%description
Storage and network testing tools including spitc, spitchecker, fsfiller,
and snack. Write and discard workloads intentionally modify their targets.

%prep
%autosetup -n stutools-%{version}

%build
cmake -S . -B build -DCMAKE_BUILD_TYPE=RelWithDebInfo \
    -DCMAKE_INSTALL_PREFIX=%{_prefix} -DCMAKE_INSTALL_LIBDIR=%{_lib} \
    -DSTUTOOLS_SYSTEMD_UNIT_DIR=%{_prefix}/lib/systemd/system
cmake --build build %{?_smp_mflags}

%check
ctest --test-dir build --output-on-failure -L smoke

%install
DESTDIR=%{buildroot} cmake --install build

%files
%license LICENSE spit/TOTP-MCU/LICENSE
%license spit/qr/qrcodegen.h spit/tpmtotp/base32.c
%doc README.md INSTALL
%{_bindir}/spitc
%{_bindir}/spitchecker
%{_bindir}/fsfiller
%{_bindir}/raidcorrupt
%{_bindir}/bdinfo
%{_bindir}/dtest
%{_bindir}/hist
%{_bindir}/snack
%{_bindir}/modcat
%{_bindir}/simpmail
%{_bindir}/pcp
%{_bindir}/sat
%{_bindir}/stush
%{_bindir}/setupbrd.sh
%{_mandir}/man1/spitc.1*
%{_mandir}/man1/spitchecker.1*
%{_mandir}/man1/snack.1*
%{_prefix}/lib/systemd/system/sat.service

%changelog
* Wed Oct 21 2026 Stuart Inglis <stuart.inglis@gmail.com> - 1.1.3-1
- Package the C suite separately and install the C I/O tester as spitc.
