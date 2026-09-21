import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import java.util.LinkedHashSet;
import java.util.Set;

public class MineBotVtableEvidence extends GhidraScript {
    private void dump(long raw, DecompInterface decompiler) throws Exception {
        Address address = toAddr(raw);
        Function function = getFunctionContaining(address);
        println(String.format("\n=== TARGET 0x%08X ===", raw));
        if (function == null) {
            println("NO FUNCTION");
            return;
        }
        println("FUNCTION=" + function.getName() + " ENTRY=" + function.getEntryPoint());
        DecompileResults results = decompiler.decompileFunction(function, 60, monitor);
        if (results.decompileCompleted() && results.getDecompiledFunction() != null) {
            println(results.getDecompiledFunction().getC());
        }
    }

    @Override
    public void run() throws Exception {
        long vtable = 0x005E43E0L;
        Set<Long> interesting = new LinkedHashSet<>();
        println("PROGRAM=" + currentProgram.getName());
        for (int off = 0; off <= 0x140; off += 4) {
            long target = Integer.toUnsignedLong(getInt(toAddr(vtable + off)));
            println(String.format("VTABLE +0x%03X -> 0x%08X", off, target));
            if (target >= 0x00450000L && target < 0x00470000L) {
                interesting.add(target);
            }
        }
        DecompInterface decompiler = new DecompInterface();
        decompiler.openProgram(currentProgram);
        for (long target : interesting) dump(target, decompiler);
        decompiler.dispose();
    }
}
