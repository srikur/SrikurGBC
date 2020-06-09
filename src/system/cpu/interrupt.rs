enum Interrupts {
    InterruptVBlank = 0x01, 
    InterruptLCDStat = 0x02, 
    InterruptTimer = 0x04, 
    InterruptSerial = 0x08,
    InterruptJoypad = 0x10,
}