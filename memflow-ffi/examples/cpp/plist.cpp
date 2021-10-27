#include "memflow.hpp"
#include <stdio.h>
#include <vector>

void fmt_arch(char *arch, int n, ArchitectureIdent ident);

int main(int argc, char *argv[]) {
	log_init(1);

	Inventory *inventory = inventory_scan();

	if (!inventory) {
		printf("unable to create inventory\n");
		return 1;
	}

	printf("inventory initialized: %p\n", inventory);

	const char *conn_name = argc > 1? argv[1]: "qemu_procfs";
	const char *conn_arg = argc > 2? argv[2]: "";
	const char *os_name = argc > 3? argv[3]: "win32";
	const char *os_arg = argc > 4? argv[4]: "";

	ConnectorInstanceArcBox connector, *conn = conn_name[0] ? &connector : nullptr;

	if (conn) {
		if (inventory_create_connector(inventory, conn_name, conn_arg, &connector)) {
			printf("unable to initialize connector\n");
			inventory_free(inventory);
			return 1;
		}

		printf("connector initialized: %p\n", connector.container.instance.instance);
	}

	OsInstanceArcBox os;

	if (inventory_create_os(inventory, os_name, os_arg, conn, &os)) {
		printf("unable to initialize OS\n");
		inventory_free(inventory);
		return 1;
	}

	inventory_free(inventory);

	printf("os initialized: %p\n", os.container.instance.instance);

	auto info = os.info();
	char arch[11];
	fmt_arch(arch, sizeof(arch), info->arch);

	printf("Kernel base: %llx\nKernel size: %llx\nArchitecture: %s\n", info->base, info->size, arch);

	printf("Process List:\n");

	printf("%-4s | %-8s | %-10s | %-10s | %s\n", "Seq", "Pid", "Sys Arch", "Proc Arch", "Name");

	int i = 0;

	os.process_info_list_callback([i](ProcessInfo info) mutable {
		char sys_arch[11];
		char proc_arch[11];

		fmt_arch(sys_arch, sizeof(sys_arch), info.sys_arch);
		fmt_arch(proc_arch, sizeof(proc_arch), info.proc_arch);

		printf("%-4d | %-8d | %-10s | %-10s | %s\n", i++, info.pid, sys_arch, proc_arch, info.name);

		return true;
	});

	return 0;
}

void fmt_arch(char *arch, int n, ArchitectureIdent ident) {
	switch (ident.tag) {
		case ArchitectureIdent::Tag::ArchitectureIdent_X86:
			snprintf(arch, n, "X86_%d", ident.x86._0);
			break;
		case ArchitectureIdent::Tag::ArchitectureIdent_AArch64:
			snprintf(arch, n, "AArch64");
			break;
		default:
			snprintf(arch, n, "Unknown");
	}
}
