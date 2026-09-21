import ghidra.app.script.GhidraScript;

public class DogBotConstantsEvidence extends GhidraScript {
    @Override public void run() throws Exception {
        long[] addresses = {0x005DD464L, 0x005DD3BCL, 0x005E1FA8L, 0x005DD380L, 0x005DD390L};
        for (long address : addresses) {
            int bits = getInt(toAddr(address));
            println(String.format("0x%08X bits=0x%08X float=%f", address, bits, Float.intBitsToFloat(bits)));
        }
    }
}
