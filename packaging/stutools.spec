Name:                 exaba-spit
Epoch:                0
Version:              0.0.1
Release:              1%{?dist}
Summary:              Stu's powerful I/O tester
License:              Proprietary
URL:                  https://exaba.com

Source0: spit
Source1: spit.1

%description
Stu's powerful I/O tester

%install
rm -rf %{buildroot}

# Binary
install -D -m 0755 %{SOURCE0} %{buildroot}%{_bindir}/spit

# man
install -D -m 0644 %{SOURCE1} %{buildroot}%{_mandir}/man1/spit.1

%files
%{_bindir}/spit
%{_mandir}/man1/spit.1*

