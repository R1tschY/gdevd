all:
	cargo build --release
	cp target/release/g213d target/release/g213d.stripped
	strip target/release/g213d.stripped

install:
	install target/release/g213d.stripped /usr/local/bin/g213d
	install target/release/g213ctl /usr/local/bin/g213ctl
	install g213d-dbus.conf /etc/dbus-1/system.d/g213d-dbus.conf
	install g213d.service /etc/systemd/system/g213d.service
	install g213refresh.service /etc/systemd/system/g213refresh.service
	systemctl daemon-reload
	systemctl restart g213d