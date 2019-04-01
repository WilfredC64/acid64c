# ACID64 Console Player

ACID64 Console Player is a console application for playing C64 music files on Windows.

The player requires a network SID device to be installed such as
[JSIDDevice](https://sourceforge.net/projects/jsidplay2/files/jsiddevice/).

The player makes use of the acid64pro.dll win32 library. Since this dll is a 32-bit
library, the code of the player can only run successfully when compiled for Windows 32 bit.

### Building

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

### Usage
```
acid64c <options> <file_name>

<Options>
  -d{device_number}: set device number (1..n), default is 1
  -h{host_name}: host name or ip of network sid device, default is localhost
  -i: display STIL info if present
  -l{hvsc_location}: specify the HVSC location for song length and STIL info
  -p: print available devices
  -s{song_number}: set song number (1..n), default is start song in SID file
```

### Run

Example of how to run the application playing the music from Commando:
```
cargo run --release -- -l"C:\HVSC\C64Music" "c:\HVSC\C64Music\MUSICIANS\H\Hubbard_Rob\Commando.sid"
```

or directly from the target folder:

```
.\acid64c.exe -l"C:\HVSC\C64Music" "c:\HVSC\C64Music\MUSICIANS\H\Hubbard_Rob\Commando.sid"
```
Make sure that the acid64pro.dll is in the same folder as the acid64c.exe executable.

### Keys
During playback you can use the following keys:
```
1-9, 0: play sub tune #1-#9, #10
+: play next sub tune
-: play previous sub tune
p: pause/resume playback
Escape (ESC) key: exit program
```

### Documentation
For documentation about the acid64pro.dll library, see the [readme.txt](/library/readme.txt) file
in the library folder.

For documentation about the network SID device, see the
[Network SID Device V4](/docs/network_sid_device_v4.html) specification,
converted from the
[JSidplay2](https://sourceforge.net/p/jsidplay2/code/HEAD/tree/trunk/jsidplay2/src/main/asciidoc/netsiddev.adoc) project.

### Licensing
The source code is licensed under the GPL v3 license. License is available [here](/LICENSE).