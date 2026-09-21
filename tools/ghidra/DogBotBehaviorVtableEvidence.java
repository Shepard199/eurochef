import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.mem.Memory;
import java.util.HashSet;

public class DogBotBehaviorVtableEvidence extends GhidraScript {
    private long ptr(Memory mem, long address) throws Exception {
        return Integer.toUnsignedLong(mem.getInt(toAddr(address)));
    }

    private void decompile(long raw, DecompInterface d, HashSet<Long> seen) throws Exception {
        if (!seen.add(raw)) return;
        Function f = getFunctionContaining(toAddr(raw));
        println(String.format("\n=== FUNCTION 0x%08X ===", raw));
        if (f == null) {
            println("NO_FUNCTION");
            return;
        }
        println("FUNCTION=" + f.getName() + " ENTRY=" + f.getEntryPoint());
        DecompileResults r = d.decompileFunction(f, 60, monitor);
        if (r.decompileCompleted() && r.getDecompiledFunction() != null) {
            println(r.getDecompiledFunction().getC());
        }
    }

    @Override public void run() throws Exception {
        String[] names = {
            "DogNode_6BC20", "DogNode_6D9E0", "DogNode_6A850",
            "DogNode_6CB30", "DogNode_44EE20", "DogNode_69E20"
        };
        long[] vtables = {
            0x005E732CL, 0x005E74E8L, 0x005E6F68L,
            0x005E7428L, 0x005E1FBCL, 0x005E6E78L
        };
        int[] slots = {0x08, 0x0C, 0x10, 0x14, 0x18, 0x20, 0x30, 0x38, 0x3C, 0x40};
        Memory mem = currentProgram.getMemory();
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        HashSet<Long> seen = new HashSet<>();
        for (int i = 0; i < names.length; i++) {
            println(String.format("\n=== VTABLE %s 0x%08X ===", names[i], vtables[i]));
            for (int slot : slots) {
                long target = ptr(mem, vtables[i] + slot);
                println(String.format("+0x%02X -> 0x%08X", slot, target));
            }
            for (int slot : slots) {
                decompile(ptr(mem, vtables[i] + slot), d, seen);
            }
        }
        d.dispose();
    }
}
