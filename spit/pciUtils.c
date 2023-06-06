#include <stdio.h>
#include <stdlib.h>
#include <pci/pci.h>

#include "utils.h"


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

      char cls[1024], clw[1024], mls[1024], mlw[1024];
      
      sprintf(cls, "/sys/bus/pci/devices/%04x:%02x:%02x.%d/current_link_speed", dev->domain, dev->bus, dev->dev, dev->func);
      sprintf(mls, "/sys/bus/pci/devices/%04x:%02x:%02x.%d/max_link_speed", dev->domain, dev->bus, dev->dev, dev->func);
      
      sprintf(clw, "/sys/bus/pci/devices/%04x:%02x:%02x.%d/current_link_width", dev->domain, dev->bus, dev->dev, dev->func);
      sprintf(mlw, "/sys/bus/pci/devices/%04x:%02x:%02x.%d/max_link_width", dev->domain, dev->bus, dev->dev, dev->func);

      //      printf("%s\n", cls);
      printf("%04x:%02x:%02x.%d [v=%04x,d=%04x] numa=%d",
	     dev->domain, dev->bus, dev->dev, dev->func, dev->vendor_id, dev->device_id,
	     /*dev->device_class, dev->irq, c,*/ /*(long) dev->base_addr[0],*/  dev->numa_node);


      ///sys/bus/pci/devices/0000\:21\:00.0/max_link_speed

      double currentLinkSpeed = getValueFromFile(cls);
      double maxLinkSpeed = getValueFromFile(mls);
      printf(", speed %.0lf/%.0lf %s", currentLinkSpeed, maxLinkSpeed, (currentLinkSpeed==maxLinkSpeed)?"":"(Suboptimal)");

      double currentLinkWidth = getValueFromFile(clw);
      double maxLinkWidth = getValueFromFile(mlw);
      printf(", width = %.0lf/%.0lf %s", currentLinkWidth, maxLinkWidth, (currentLinkWidth==maxLinkWidth)?"":"(Suboptimal)");

      printf(", %.1lf Gb/s", currentLinkWidth * currentLinkSpeed * 128.0/130);

      /* Look up and print the full name of the device */
      char namebuf[1000];
      char *name = pci_lookup_name(pacc, namebuf, sizeof(namebuf), PCI_LOOKUP_DEVICE, dev->vendor_id, dev->device_id);
      printf(" (%s)\n", name);

    }

    pci_cleanup(pacc);
}
