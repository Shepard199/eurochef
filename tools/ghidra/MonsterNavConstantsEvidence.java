import ghidra.app.script.GhidraScript;
import ghidra.program.model.mem.Memory;

public class MonsterNavConstantsEvidence extends GhidraScript {
    @Override public void run() throws Exception {
        Memory m = currentProgram.getMemory();
        long[] addrs = new long[]{0x005DD380L,0x005DD384L,0x005DD388L,0x005DD938L,0x005E1F94L};
        for (long a : addrs) {
            int bits = m.getInt(toAddr(a));
            println(String.format("0x%08X bits=0x%08X float=%s", a, bits, Float.intBitsToFloat(bits)));
        }
    }
}
