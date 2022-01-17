all:
	cargo build --release
	cp target/release/gdevd target/release/gdevd.stripped
	strip target/release/gdevd.stripped

install:
	install target/release/gdevd.stripped /usr/local/bin/gdevd
	install target/release/gdevctl /usr/local/bin/gdevctl
	install gdevd-dbus.conf /etc/dbus-1/system.d/gdevd-dbus.conf
	install gdevd.service /etc/systemd/system/gdevd.service
	install gdevrefresh.service /etc/systemd/system/gdevrefresh.service
	systemctl daemon-reload
	systemctl restart gdevd

uninstall:
	systemctl stop gdevd || true
	systemctl clean gdevd || true
	systemctl daemon-reload || true
	rm /usr/local/bin/gdevd 2> /dev/null || true
	rm /usr/local/bin/gdevctl 2> /dev/null || true
	rm /etc/dbus-1/system.d/gdevd-dbus.conf 2> /dev/null || true
	rm /etc/systemd/system/gdevd.service 2> /dev/null || true
	rm /etc/systemd/system/gdevrefresh.service 2> /dev/null || true
