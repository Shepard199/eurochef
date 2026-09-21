import ghidra.app.script.GhidraScript;
import ghidra.program.model.mem.Memory;

public class HitQuerySerialEvidence extends GhidraScript {
    @Override public void run() throws Exception {
        Memory mem = currentProgram.getMemory();
        short value = mem.getShort(toAddr(0x00616DA8L));
        println(String.format("DAT_00616DA8 image u16=0x%04X signed=%d", value & 0xffff, (int)value));
    }
}
