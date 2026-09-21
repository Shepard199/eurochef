import ghidra.app.script.GhidraScript;

public class TestAnimConstantsEvidence extends GhidraScript {
    @Override
    public void run() throws Exception {
        long[] addrs = {
            0x005E2190L, 0x005E2194L,
            0x005DD72CL, 0x00620034L, 0x005DD384L, 0x005DD388L, 0x005DE534L
        };
        for (long a : addrs) {
            int raw = getInt(toAddr(a));
            println(String.format("0x%08X raw=0x%08X float=%s", a, raw, Float.intBitsToFloat(raw)));
        }
    }
}
