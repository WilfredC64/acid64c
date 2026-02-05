# ACID64 Console Player

ACID64 Console Player is a console application for playing C64 music files on Windows.

The player requires a network SID device to be installed (such as
[SID-Device](https://github.com/WilfredC64/sid-device/) or
[JSIDDevice](https://sourceforge.net/projects/jsidplay2/files/jsiddevice/)) or
a hardware device such as a USBSID-Pico, HardSID USB, SIDBlaster USB, Commodore 64 Ultimate or an Ultimate 64/1541.

The player makes use of the acid64pro.dll win32 library. Since this dll is a 32-bit
library, the code of the player can only run successfully when compiled for Windows 32-bit.

## Building

In order to build the project, make sure you have installed one of the following ABIs
via rustup:

```
i686-pc-windows-msvc

i686-pc-windows-gnu
```
Set one of the ABIs to the default via rustup and add it to the .cargo/config file.

For building:

```
cargo build --release
```

## Usage
```
acid64c <options> <file_name>

<Options>
  -c: adjust clock for devices that don't support PAL/NTSC clock
  -d{device_number,n}: set device numbers (1..n) for each SID chip, default is 1
  -hs{host_name}: host name or IP of network sid device, default is localhost
  -hu{ip_address}: IP of Ultimate device
  -i: display STIL info if present
  -l{hvsc_location}: specify the HVSC location for song length and STIL info
  -p: print available devices
  -s{song_number}: set song number (1..n), default is start song in SID file
```

## Run

Example of how to run the application playing the music from Commando:
```
cargo run --release -- -l"C:\HVSC\C64Music" "c:\HVSC\C64Music\MUSICIANS\H\Hubbard_Rob\Commando.sid"
```

or directly from the target folder:

```
.\acid64c.exe -l"C:\HVSC\C64Music" "c:\HVSC\C64Music\MUSICIANS\H\Hubbard_Rob\Commando.sid"
```
Make sure that the acid64pro.dll is in the same folder as the acid64c.exe executable.

## Keys
During playback, you can use the following keys:
```
1-9, 0: play sub tune #1-#9, #10
+: play next sub tune
-: play previous sub tune
p: pause/resume playback
Cursor-Left: Disable fast forward
Cursor-Right: Toggle fast forward
Escape (ESC) key: exit program
```

## Documentation
For documentation about the acid64pro.dll library, see the [readme.txt](/library/readme.txt) file
in the library folder.

For documentation about the network SID device, see the
[Network SID Device V4](https://htmlpreview.github.io/?https://github.com/WilfredC64/acid64c/blob/master/docs/network_sid_device_v4.html) specification,
converted from the
[JSidplay2](https://sourceforge.net/p/jsidplay2/code/HEAD/tree/trunk/jsidplay2/src/main/asciidoc/netsiddev.adoc) project.

## USBSID-Pico support

ACID64 supports the USBSID-Pico device. For Windows, you need to have a driver installed.

### Driver installation

You need to install the WinUSB driver which is available via the Zadig tool:

[https://zadig.akeo.ie/](https://zadig.akeo.ie/)


This is an open-source tool which will install a generic signed driver that can control any USB device.

When you run the Zadig tool, turn on your device and see if one of the following devices are in the
list:

- USBSID-Pico Data (Interface 0)

If your device is not in the dropdown list, check if it is connected and turned
it on. You can also go to the "Options" menu and select "List All Devices" and
see if the device is in the list.

Now find and select the USBSID-Pico device from the dropdown. Notice the USB ID is:

- **CAFE 4011 00** for USBSID-Pico Data (Interface 0)

Don't select the USBSID-Pico with "WebUSB" or "Midi" in the name. You need to
select the one with "Data" in the name.

Make sure the WinUSB driver is selected and press the "Install Driver" button.
It will take a while before the installation completes.

You have to install the driver for each USBSID-Pico Device that you plug in.

## HardSID USB support

ACID64 supports HardSID USB devices like the **HardSID 4U**, **HardSID UPlay** and **HardSID Uno**.
For this you need to have a driver installed.
ACID64 supports the official HardSID Windows driver and the WinUSB driver.

### Driver installation

On Windows, it's recommended to install the WinUSB driver,
since it is digitally signed and can be used without any tricks.

To install the WinUSB driver, just download the Zadig tool via:

[https://zadig.akeo.ie/](https://zadig.akeo.ie/)


This is an open-source tool which will install a generic signed driver that can control any USB device.

Before installing the driver via the Zadig tool, make sure to uninstall the official HardSID driver
if you already have it installed. Connect and turn on your HardSID USB device and go to
**Computer Management** to uninstall the driver.
Also make sure you select the **Delete the driver software for this device** during uninstall and reboot when done.

When you run the Zadig tool, turn on your device and see if one of the following devices are in the list:

- HardSID 4U
- HardSID UPlay
- HardSID Uno

If your device is not in the dropdown list, check if it is connected and turned it on.
You can also go to the **Options** menu and select **List All Devices** and see if the HardSID
device is in the list. If it is, the device still has a driver assigned and you need to uninstall it, or
you forgot to reboot. Just follow the procedure above again.

Now find and select the HardSID device from the dropdown. Notice the USB ID is:

- **6581 8580** for the HardSID 4U
- **6581 8581** for the HardSID UPlay
- **6581 8582** for the HardSID Uno

Make sure the WinUSB driver is selected and press the **Install Driver** button.
It will take a while before the installation completes.

You have to install the driver for each type of USB HardSID Device that you plug in.

## Licensing
The source code is licensed under the GPL v3 license. License is available [here](/LICENSE).
