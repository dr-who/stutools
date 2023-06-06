#include <stdio.h>
#include <stdlib.h>
#include <pci/pci.h>



void listPCIdevices(size_t classfilter) {
    struct pci_access *pacc;
    struct pci_dev *dev;
    u8 cfg_space[4096] = {0};

    pacc = pci_alloc();  /* Get the pci_access structure */

    /* Set all options you want -- here we stick with the defaults */

    pci_init(pacc);     /* Initialize the PCI library */
    pci_scan_bus(pacc); /* We want to get the list of devices */

    for (dev=pacc->devices; dev; dev=dev->next) { /* Iterate over all devices */

      if (!pci_read_block(dev, 0x0, cfg_space, 2)) {
	// handle error
	return;
      }

      pci_fill_info(dev, -1);
      //      unsigned int c = pci_read_byte(dev, PCI_INTERRUPT_PIN);                                /* Read config register directly */

      if ((dev->device_class & 0xff00) != classfilter) {
	continue;
      }

      char str[100], strw[100];
      sprintf(str, "/sys/bus/pci/devices/%04x:%02x:%02x.%d/current_link_speed", dev->domain, dev->bus, dev->dev, dev->func);
      sprintf(strw, "/sys/bus/pci/devices/%04x:%02x:%02x.%d/current_link_width", dev->domain, dev->bus, dev->dev, dev->func);

      //      printf("%s\n", str);
      printf("%04x:%02x:%02x.%d [v=%04x,d=%04x] numa=%d",
	     dev->domain, dev->bus, dev->dev, dev->func, dev->vendor_id, dev->device_id,
	     /*dev->device_class, dev->irq, c,*/ /*(long) dev->base_addr[0],*/  dev->numa_node);


      ///sys/bus/pci/devices/0000\:21\:00.0/max_link_speed

      float linkspeed = 1, linkwidth = 1;
      
      FILE *fp = fopen (str, "rt");
      if (fp) {
	char result[100];
	int ret = fscanf(fp, "%s", result);
	if (ret == 1) {
	  linkspeed = atof(result);
	}
	
	printf(", speed %s", result);
	fclose(fp);
      } else {
	//	perror(str);
      }

      fp = fopen (strw, "rt");
      if (fp) {
	char result[100];
	int ret = fscanf(fp, "%s", result);
	if (ret == 1) {
	  linkwidth = atof(result);
	}
	printf(", width = %s", result);
	fclose(fp);
      } else {
	//	perror(str);
      }

      printf(", %.1lf Gb/s", linkwidth * linkspeed * 128.0/130);

      /* Look up and print the full name of the device */
      char namebuf[1000];
      char *name = pci_lookup_name(pacc, namebuf, sizeof(namebuf), PCI_LOOKUP_DEVICE, dev->vendor_id, dev->device_id);
      printf(" (%s)\n", name);

    }

    pci_cleanup(pacc);
}
