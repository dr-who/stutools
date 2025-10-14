Name:                 exaba-spit
Epoch:                0
Version:              1.1.3
Release:              1%{?dist}
Summary:              Stu's powerful I/O tester
License:              Proprietary
URL:                  https://exaba.com

Source0: spit
Source1: spit.1
Source2: snack
Source3: snack.1

%description
Stu's powerful I/O tester. spit is the tool for block devices. snack tests network speeds.

%install
rm -rf %{buildroot}

# Binary
install -D -m 0755 %{SOURCE0} %{buildroot}%{_bindir}/spit
install -D -m 0755 %{SOURCE2} %{buildroot}%{_bindir}/snack

# man
install -D -m 0644 %{SOURCE1} %{buildroot}%{_mandir}/man1/spit.1
install -D -m 0644 %{SOURCE3} %{buildroot}%{_mandir}/man1/snack.3

%files
%{_bindir}/spit
%{_bindir}/snack
%{_mandir}/man1/spit.1*
%{_mandir}/man1/snack.1*

