
                    ACID 64 Player Library v2.0.6

                 Copyright (c) 2008-2020 Wilfred Bos
                     Programmed by: Wilfred Bos
                       https://www.acid64.com
                Concept by: Sándor Téli & Wilfred Bos


USE THIS LIBRARY AT YOUR OWN RISK. THIS LIBRARY COMES WITHOUT WARRANTY
OF ANY KIND. THE AUTHOR IS NOT LIABLE FOR ANY DAMAGE IN ANY EVENT AS A
RESULT OF USING THIS SOFTWARE.


The library supports the following methods (in Delphi code):

  function getVersion(): Integer; stdcall; external 'acid64pro';

  function createC64Instance(): Pointer; stdcall; external 'acid64pro';
  procedure closeC64Instance(c64: Pointer); stdcall; external 'acid64pro';

  function checkSldb(filename: PChar): Boolean; stdcall; external 'acid64pro';
  function checkSldbFromBuffer(buffer: Pointer; size: Integer): Boolean; stdcall; external 'acid64pro';
  function loadSldb(filename: PChar): Boolean; stdcall; external 'acid64pro';
  function loadSldbFromBuffer(buffer: Pointer; size: Integer): Boolean; stdcall; external 'acid64pro';
  function getFilename(md5Hash: PChar): PChar; stdcall; external 'acid64pro';
  function loadStil(hvscLocation: PChar): Boolean; stdcall; external 'acid64pro';
  function loadStilFromBuffer(buffer: Pointer; size: Integer): Boolean; stdcall; external 'acid64pro';

  // methods that need a C64 instance
  procedure run(c64: Pointer); stdcall; external 'acid64pro';
  function loadFile(c64: Pointer; filename: PChar): Boolean; stdcall; external 'acid64pro';
  function getCommand(c64: Pointer): Integer; stdcall; external 'acid64pro';
  function getRegister(c64: Pointer): Byte; stdcall; external 'acid64pro';
  function getData(c64: Pointer): Byte; stdcall; external 'acid64pro';
  function getCycles(c64: Pointer): Word; stdcall; external 'acid64pro';
  function getTitle(c64: Pointer): PChar; stdcall; external 'acid64pro';
  function getAuthor(c64: Pointer): PChar; stdcall; external 'acid64pro';
  function getReleased(c64: Pointer): PChar; stdcall; external 'acid64pro';
  function getNumberOfSongs(c64: Pointer): Integer; stdcall; external 'acid64pro';
  function getDefaultSong(c64: Pointer): Integer; stdcall; external 'acid64pro';
  function getLoadAddress(c64: Pointer): Integer; stdcall; external 'acid64pro';
  function getLoadEndAddress(c64: Pointer): Integer; stdcall; external 'acid64pro';
  function getPlayAddress(c64: Pointer): Integer; stdcall; external 'acid64pro';
  function getInitAddress(c64: Pointer): Integer; stdcall; external 'acid64pro';
  function getSidModel(c64: Pointer, sidNr: Integer): Integer; stdcall; external 'acid64pro';
  function getC64Version(c64: Pointer): Integer; stdcall; external 'acid64pro';
  function getTime(c64: Pointer): LongWord; stdcall; external 'acid64pro';
  function getSongLength(c64: Pointer): Integer; stdcall; external 'acid64pro';
  function getMd5Hash(c64: Pointer): PChar; stdcall; external 'acid64pro';
  function getStilEntry(c64: Pointer): PChar; stdcall; external 'acid64pro';
  procedure setSongToPlay(c64: Pointer; songToPlay: Integer); stdcall; external 'acid64pro';
  procedure setC64Version(c64: Pointer; c64Version: Integer); stdcall; external 'acid64pro';
  procedure pressButtons(c64: Pointer); stdcall; external 'acid64pro';
  procedure enableFixedStartup(c64: Pointer); stdcall; external 'acid64pro';
  procedure skipSilence(c64: Pointer; enabled: Boolean); stdcall; external 'acid64pro';
  procedure enableVolumeFix(c64: Pointer; enabled: Boolean); stdcall; external 'acid64pro';
  procedure getMemoryUsageRam(c64: Pointer; pBuffer: Pointer; size: Integer); stdcall; external 'acid64pro';
  procedure getMemoryUsageRom(c64: Pointer; pBuffer: Pointer; size: Integer); stdcall; external 'acid64pro';
  procedure getMemory(c64: Pointer; pBuffer: Pointer; size: Integer); stdcall; external 'acid64pro';
  procedure clearMemUsageOnFirstSidAccess(c64: Pointer; blnClear: Boolean); stdcall; external 'acid64pro';
  procedure clearMemUsageAfterInit(c64: Pointer; blnClear: Boolean); stdcall; external 'acid64pro';
  function getNumberOfSids(c64: Pointer): Integer; stdcall; external 'acid64pro';
  function getAncientMd5Hash(c64: Pointer): PChar; stdcall; external 'acid64pro';
  procedure startSeek(time: LongWord); stdcall; external 'acid64pro';
  procedure stopSeek(); stdcall; external 'acid64pro';
  function getCpuLoad(): Integer; stdcall; external 'acid64pro';
  function getSpeedFlag(): Integer; stdcall; external 'acid64pro';
  function getSpeedFlags(c64: Pointer): Integer; stdcall; external 'acid64pro';
  function getFrequency(): Integer; stdcall; external 'acid64pro';
  function getFileType(c64: Pointer): PChar; stdcall; external 'acid64pro';
  function getFileFormat(c64: Pointer): PChar; stdcall; external 'acid64pro';
  procedure getMusText(c64: Pointer; pBuffer: Pointer; size: Integer); stdcall; external 'acid64pro';
  procedure getMusColors(c64: Pointer; pBuffer: Pointer; size: Integer); stdcall; external 'acid64pro';
  function isBasicSid(c64: Pointer): Boolean; stdcall; external 'acid64pro';
  function getSidAddress(c64: Pointer; sidNr: Integer): Integer; stdcall; external 'acid64pro';
  function getFreeMemoryAddress(c64: Pointer): Integer; stdcall; external 'acid64pro';
  function getFreeMemoryEndAddress(c64: Pointer): Integer; stdcall; external 'acid64pro';


getVersion
==========
The 'getVersion' method returns the version number of the library in hex.
E.g. 0x206 means version 2.06.


createC64Instance
=================
To initialize the emulator you have to call the 'createC64Instance' method. This
method returns a reference of the instance of the C64. You can create up to 256
C64 instances that can run simultaneously. By this you can run multiple threads,
each running a particular instance. Be aware not to share the same instance by
multiple threads. For each thread you have to create another instance. If an
instance isn't needed anymore then you should call the 'closeC64Instance'
method.

The 'createC64Instance' method returns null when an error occurs or when an
instance can't be created.


closeC64Instance
================
The 'closeC64Instance' method closes a C64 instance. You have to pass the
reference of the C64 instance which you want to close.


loadSldb
========
The 'loadSldb' method loads the song length database file into memory for all
the C64 instances. If the file can't be loaded then the method returns false
otherwise true. It is not necessary to call the checkSldb method first since the
loadSldb is calling the checkSldb method internally.

The passed filename can be a file or directory. If it is a directory it tries to
load the file 'Songlengths.md5' if present and otherwise it tries to load the
file 'Songlengths.txt'. If you pass the HVSC folder, it will retrieve the
'Songlengths.md5' file there automatically or when the md5 file is not present
the 'Songlengths.txt' file.


loadSldbFromBuffer
==================
The 'loadSldbFromBuffer' method loads the song length database passed via a
buffer for all the C64 instances. If the data can't be loaded then the method
returns false otherwise true. It is not necessary to call the
checkSldbFromBuffer method first since the loadSldbFromBuffer is performing the
check as well.

The passed pointer is the pointer to the buffer where the data of the SLDB is
present. The passed size is the size of the buffer.


checkSldb
=========
The 'checkSldb' method checks if the SLDB file exists and if it is a valid song
length database file. If the file is valid then the method returns true
otherwise false.

The passed filename can be a file or directory. If it is a directory it tries to
load the file 'Songlengths.md5' if present and otherwise it tries to load the
file 'Songlengths.txt'. If you pass the HVSC folder, it will retrieve the
'Songlengths.md5' file there automatically or when the md5 file is not present
the 'Songlengths.txt' file.


checkSldbFromBuffer
===================
The 'checkSldbFromBuffer' method checks if the SLDB data in the buffer is valid.
If the data is valid then the method returns true otherwise false.

The passed pointer is the pointer to the buffer where the data of the SLDB is
present. The passed size is the size of the buffer.


loadStil
========
The 'loadStil' method loads the SID Tune Information List (STIL) file into
memory that is located at the High Voltage SID Collection (HVSC) location. The
info is loaded for all the C64 instances. If the file can't be loaded then the
method returns false otherwise true.


loadStilFromBuffer
==================
The 'loadSTILFromBuffer' method loads the SID Tune Information List (STIL) data
passed via a buffer. The info is loaded for all the C64 instances. If the data
is invalid then the method returns false otherwise true.


getFileName
===========
The 'getFileName' method can be used to retrieve the filename from a MD5 hash.
The method can be used without a c64 instance. In order to use the 'getFileName'
method you should first load the Song Length Database (SLDB) with method
'loadSldb' since the info is retrieved from the SLDB data.


run
===
The 'run' method runs the loaded C64 program for a number of cycles.


loadFile
========
To run a SID tune (or prg/p00/mus file) you first have to call the 'loadFile'
method and pass a null terminated string to it that includes the path and
filename. After that, you can use all the other methods that requires a C64
instance.

The 'loadFile' method returns a boolean that indicates if loading the tune was
successful.


getCommand
==========
The 'getCommand' method returns an integer that indicates a command after
calling the run method.

The following constants can be used for the integer values that the method can
return:

  SID_IDLE_COMMAND = 0;
  SID_DELAY_COMMAND = 1;
  SID_WRITE_COMMAND = 2;
  SID_READ_COMMAND = 3;
  NEXT_PART_COMMAND = 4;
  SID_INIT_DONE_COMMAND = 5;
  SID_SEEK_DONE_COMMAND = 6;

To run the emulator and control a SID device the following code can be used:

  Pointer c64 = createC64Instance();
  if (c64 != null) {
    try {
      if (loadFile(c64, filename) == true) {
        while (true) {
          run(c64);
          sidCommand = getCommand(c64);

          switch (sidCommand) {
            case SID_DELAY_COMMAND:
              deviceDelay(deviceNum, getCycles(c64));
              break;
            case SID_WRITE_COMMAND:
              deviceWrite(deviceNum, getCycles(c64), getRegister(c64), getData(c64));
              break;
          }
        }
      }
    } finally {
      closeC64Instance(c64);
    }
  }

When the duration between SID writes is greater than 0xffff, the 'getCommand'
method will return SID_DELAY_COMMAND with cycles set to 0xffff. When the
remaining cycles is less than or equal to 0xffff the 'getCommand' method will
return SID_WRITE_COMMAND with the remaining cycles set. The 'getCommand' method
will return SID_IDLE_COMMAND when nothing is written to the SID for the last
millisecond. This means that the run method emulates maximum 1 millisecond of
the 6510 clock.

The SID_IDLE_COMMAND command is returned when the 'run' method didn't process
any reads from or writes to the SID.

The SID_READ_COMMAND command can only be used for information only. You can call
the 'getCycles' method when the command is processed to see at which cycle the
read command is performed. The cycles returned is also for information only
since the number of cycles between the last write and the following write
(including the read command cycles) is returned by the 'getCycles' when the
SID_WRITE_COMMAND is processed.


getTitle
========
The 'getTitle' method returns the title of the SID file.


getAuthor
=========
The 'getAuthor' method returns the author of the SID file.


getReleased
===========
The 'getReleased' method returns the released info (year and publisher) of the
SID file.


getNumberOfSongs
================
The 'getNumberOfSongs' method returns the number of songs a SID file has
defined. The minimum value that is returned is 1.


getDefaultSong
==============
The 'getStartSong' method returns the default song number that is defined in the
SID file.
The minimum value that can be returned is 0 which indicated the first song.
You can play the default song by doing the following:

int defaultSong = getDefaultSong();
setSongToPlay(defaultSong);


setSongToPlay
=============
The 'setSongToPlay' method sets the song number to be played. The song number
has a minimum value of 0 and a maximum value that is retrieved by the
'getNumberOfSongs' method minus 1.

When this method isn't called, the default song specified in the SID header
will be played.

When a song of a SID file is already played and the 'setSongToPlay' is called
for selecting a new song within the SID file, the SID file will be reloaded
automatically and the tune will be started from the beginning.


getTime
=======
The 'getTime' method returns the passed time in milliseconds. This can be used
to display time information about the SID being played. Just call the method
every second during running the SID tune.


getSidModel
===========
The 'getSidModel' method returns an integer that indicates the SID model for the
specified SID chip.

You can interpret the returned value as follows:

  intSidModel := getSidModel(sidNr);
  case intSidModel of
    0: model := 'Unknown';
    1: model := 'MOS 6581';
    2: model := 'MOS 8580';
    3: model := 'MOS 6581/8580';
  end;


getC64Version
=============
The 'getC64Version' method returns an integer that indicates the C64 clock
version of the CPU and can be interpreted like:

  intC64Version := getC64Version();
  case intC64Version of
    0: c64Version := 'Unknown';
    1: c64Version := 'PAL';
    2: c64Version := 'NTSC';
    3: c64Version := 'PAL/NTSC';
  end;


getMd5Hash
==========
The 'getMd5Hash' method generates the MD5 hash of the current loaded file. The
MD5 hash can be used to identify a SID file. Note that the MD5 generation of a
SID file has been changed as of HVSC version #68. To get the MD5 hash that is
generated via the old way, just call the getAncientMd5Hash.


getAncientMd5Hash
=================
The 'getAncientMd5Hash' method generates the MD5 hash of the current loaded file
in the old way. The MD5 hash can be used to identify a SID file.


getSongLength
=============
When you do 'getSongLength', just set the song number first with 'setSongToPlay'
to get the song length of the specified song. The song length is in
milliseconds. In order to use the 'getSongLength' method you should first load
the Song Length Database (SLDB) with method 'loadSldb'.


getStilEntry
============
The 'getStilEntry' method retrieves the STIL info for the current loaded file
for a C64 instance. If the info isn't found then the method will return null,
otherwise it returns a null terminated string. In order to use the
'getStilEntry' method you should first load the STIL info via method 'loadSTIL'.

Be aware that when the SLDB is not loaded, only the STIL info is retrieved for
songs located in the HVSC location that is provided with the loadSTIL method.


skipSilence
===========
The 'skipSilence' method can be used to skip silence at the beginning of a SID
tune. This feature is default turned off. You can enable this feature per C64
instance. If enabled then the SID data will be analysed and checked when the
first audible note is written. All the data that is written to the SID is still
returned, only the cycle data is modified so that the tune starts immediately.

In order to know how much time is skipped, you can simply use the getTime method
when the first SID_WRITE_COMMAND is triggered. You can then subtract the time
from the total amount that is retrieved from getSongLength, or display the
current time that is retrieved by getTime.

This method should be called before the 'run' method or after the
'setSongToPlay' method.


enableVolumeFix
===============
The 'enableVolumeFix' method will fix all tunes that doesn't set the
volume/filter ($D418) register. This feature can be used to avoid silence for
buggy tunes. It is by default turned off. You can enable this feature per C64
instance. If this feature is enabled, then you don't have to turn on the volume
manually, it will be done automatically as if it is done by the C64 player
itself.

This method should be called before the 'run' method or after the
'setSongToPlay' method.


pressButtons
============
The 'pressButtons' method will virtually press space bar and all the joystick
buttons for about 150 milliseconds and then release them. It can be used to skip
intros or for playing all the music for a particular demo.

After the 'pressButtons' method is called, a NEXT_PART_COMMAND can be captured
which will be generated when a program has processed the space bar or joystick
button press.


enableFixedStartup
==================
The 'enableFixedStartup' method will make sure the C64 is started exactly the
same for every tune. This means that it will not have a random start-up time,
which is required for certain E-Loader tunes. It is by default turned off.

This method should be called before the 'run' method or after the
'setSongToPlay' method.


getMemoryUsageRam
=================
The 'getMemoryUsageRam' method will retrieve a 64KB memory map of the RAM usage.

The following bit values are specified for each memory byte:

  MEM_EXEC = $80;
  MEM_DUMMY_READ = $40;
  MEM_READ = $20;
  MEM_BAD_READ = $10;
  MEM_WRITE = $08;
  MEM_WRITE_FIRST = $04;
  MEM_LOAD = $02;
  MEM_UNUSED = $00;

The passed pointer is the pointer to the buffer where the data of the memory
usage should be written to. The passed size is the size of the buffer. For the
full map, the size should be set to 65536 (64KB).


getMemoryUsageRom
=================
The 'getMemoryUsageRom' method will retrieve a 64KB memory map of the ROM usage.

The following bit values are specified for each memory byte:

  MEM_EXEC = $80;
  MEM_DUMMY_READ = $40;
  MEM_READ = $20;
  MEM_WRITE = $08;
  MEM_LOAD = $02;
  MEM_UNUSED = $00;

The passed pointer is the pointer to the buffer where the data of the memory
usage should be written to. The passed size is the size of the buffer. For the
full map, the size should be set to 65536 (64KB).


getLoadAddress
==============
The 'getLoadAddress' method will retrieve the load address of the SID tune.


getLoadEndAddress
=================
The 'getLoadEndAddress' method will retrieve the load end address of the SID
tune. The load end address is address of the last byte of the SID tune + 1.


getInitAddress
==============
The 'getInitAddress' method will retrieve the init address of the SID tune.


getPlayAddress
==============
The 'getPlayAddress' method will retrieve the play address of the SID tune.


setC64Version
=============
The 'setC64Version' method sets the C64 clock version of the CPU. This method
can be used to overwrite the C64 clock version of the SID that is specified in
the SID header.

Possible values:
    0 = 'Unknown'
    1 = 'PAL'
    2 = 'NTSC'
    3 = 'PAL/NTSC'

This method should be called before the 'run' method or after the
'setSongToPlay' method.


clearMemUsageOnFirstSidAccess
=============================
The 'clearMemUsageOnFirstSidAccess' method will enable or disable the option to
clear the memory usage when the SID is accessed for the first time. By default
it is set to false.

The option can be used when you want to start measuring memory usage when the
SID is accessed. Via this you can skip memory usage of e.g. decrunchers that
doesn't access the SID chip.


clearMemUsageAfterInit
======================
The 'clearMemUsageAfterInit' method will enable or disable the option to clear
the memory usage when the init routine of the SID file is finished. By default
it is set to false. This method has only impact on PSID tunes.


getMemory
=========
The 'getMemory' method will retrieve the memory data from RAM.

The passed pointer is the pointer to the buffer where the data of the memory
should be written to. The passed size is the size of the buffer. For the full
memory, the size should be set to 65536 (64KB).


getNumberOfSids
===============
The 'getNumberOfSids' method returns the number of SIDs that are defined for the
tune. Call this method after calling the 'loadFile' method.

If the method returns 1, then all writes to the SID chip are mapped to registers
$00-$1F.

If the method returns 2, then all writes to the second SID chip are mapped to
registers $20-$3F.

If the method returns 3, then all writes to the third SID chip are mapped to
registers $40-$5F.

If e.g. the second SID address in the SID header is set to $D500, then is will
be mapped to $D420-$D43F internally. Make sure you write the data for each SID
mapping to a different device.


startSeek
=========
The 'startSeek' method seeks the sid tune to the given time. The time is given
in milliseconds. During the seek you need to keep calling the 'run' method. The
getCommand method will not return any write, read, delay or idle commands. When
the seek is done, the getCommand method will return SID_SEEK_DONE_COMMAND.


stopSeek
========
With the 'stopSeek' method you will be able to stop the seek action. Normally
this is automatically done when the seek is done, but in case you want to
cancel it before the time is reached, you can use this method.
Note that the seek action is cancelled when the loadFile is called. In this
cause you don't have to call stopSeek.


getCpuLoad
==========
The 'getCpuLoad' method returns the CPU load in percentages rounded as a whole
number. RSID tunes that play in a loop will always have 100% CPU load.


getSpeedFlag
============
The 'getSpeedFlag' method returns the speed flag of the current selected song.

Possible values:
    0 = VBI IRQ
        For RSID tunes it is always 0
        For PSID tunes it means 50 Hz for PAL, 60 Hz for NTSC.
    1 = CIA IRQ
        Default 60 Hz but the CIA timer can be overwritten by the tune.


getSpeedFlags
=============
The 'getSpeedFlags' method returns all the speed flags defined in the SID file.
See 'speed' field in the SID File Format specification for details.


getFrequency
============
The 'getFrequency' method returns the highest frequency of the 4 CIA timers in
hertz when the speed flag is set to CIA IRQ. When the speed flag is set to VBI
IRQ, then it will return the frequency in hertz of the VBI IRQ.


getFileType
===========
The 'getFileType' method returns the file type of the loaded file.

Possible values:

SID = SID file
MUS = Sidplayer64 MUS file
PRG = C64 Program file


getFileFormat
=============
The 'getFileFormat' method returns the file format of the loaded file as text.


getMusText
==========
The 'getMusText' receives the MUS text in PETSCII.

The passed pointer is the pointer to the buffer where the MUS text will be
written to. The passed size is the size of the buffer. The buffer size should
be minimal 32 * 5 in size since the text can be 5 lines of each containing 32
characters.


getMusColors
============
The 'getMusColors' receives the colors of the MUS text.

The passed pointer is the pointer to the buffer where the MUS colors will be
written to. The passed size is the size of the buffer. The buffer size should
be minimal 32 * 5 in size since the color data can be 5 lines of each
containing 32 bytes.

The color data exists of a byte that can have a value from 0 to 15 which
represents the color code.


isBasicSid
==========
The 'isBasicSid' returns a boolean which indicates if the SID file uses BASIC
to play the music.


getSidAddress
=============
The 'getSidAddress' method will retrieve the SID address location of the
specified SID chip.

To get the addresses of each SID chip used in the SID tune, you can call the
'getNumberOfSids' method first to get the number of SIDs and then call the
'getSidAddress' for each SID chip.


getFreeMemoryAddress
====================
The 'getFreeMemoryAddress' method will retrieve the memory location of the free
area that is not used by the SID tune. To get the end address of the area you
can call the 'getFreeMemoryEndAddress' method.


getFreeMemoryEndAddress
=======================
The 'getFreeMemoryEndAddress' method will retrieve the memory location of the
end of the free area that is not used by the SID tune. To get the start address
of the area you can call the 'getFreeMemoryAddress' method.
