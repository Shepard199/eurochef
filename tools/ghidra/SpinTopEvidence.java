import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;

public class SpinTopEvidence extends GhidraScript {
    private void decompile(long raw, DecompInterface decompiler) throws Exception {
        Function function = getFunctionAt(toAddr(raw));
        if (function == null) {
            function = getFunctionContaining(toAddr(raw));
        }
        println(String.format("\n=== 0x%08X %s ===", raw, function == null ? "<missing>" : function.getName()));
        if (function == null) return;
        DecompileResults result = decompiler.decompileFunction(function, 60, monitor);
        if (result.decompileCompleted() && result.getDecompiledFunction() != null) {
            println(result.getDecompiledFunction().getC());
        }
    }

    @Override
    public void run() throws Exception {
        println("SpinTop vtable 0x005E4270");
        int[] slots = {0x08, 0x34, 0x108};
        for (int slot : slots) {
            long target = Integer.toUnsignedLong(getInt(toAddr(0x005E4270L + slot)));
            println(String.format("+0x%03X -> 0x%08X", slot, target));
        }

        DecompInterface decompiler = new DecompInterface();
        decompiler.openProgram(currentProgram);
        decompile(0x0045F910L, decompiler);
        decompile(0x0045F960L, decompiler);
        decompile(0x0045FC20L, decompiler);
        decompiler.dispose();
    }
}
