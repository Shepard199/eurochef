import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.symbol.Reference;

public class PiranhaTestAnimDeepEvidence extends GhidraScript {
    private long ptr(long a) throws Exception {
        return Integer.toUnsignedLong(getInt(toAddr(a)));
    }

    private void dumpLinear(long raw, int maxInstructions) throws Exception {
        Address start = toAddr(raw);
        disassemble(start);
        println(String.format("\n=== LINEAR 0x%08X ===", raw));
        InstructionIterator it = currentProgram.getListing().getInstructions(start, true);
        int count = 0;
        while (it.hasNext() && count < maxInstructions) {
            Instruction ins = it.next();
            println(ins.getAddress() + "  " + ins.toString());
            for (Reference ref : ins.getReferencesFrom()) {
                println("    REF " + ref.getReferenceType() + " -> " + ref.getToAddress());
            }
            count++;
        }
    }

    private void decompile(long raw, DecompInterface d) throws Exception {
        Function f = getFunctionAt(toAddr(raw));
        if (f == null) f = getFunctionContaining(toAddr(raw));
        println(String.format("\n=== DECOMPILE 0x%08X %s ===", raw, f == null ? "<missing>" : f.getName()));
        if (f == null) return;
        DecompileResults r = d.decompileFunction(f, 60, monitor);
        if (r.decompileCompleted() && r.getDecompiledFunction() != null) {
            println(r.getDecompiledFunction().getC());
        }
    }

    private void vtable(String name, long raw, DecompInterface d) throws Exception {
        println(String.format("\n=== %s VTABLE 0x%08X ===", name, raw));
        for (int off = 0; off <= 0x40; off += 4) {
            long f = ptr(raw + off);
            println(String.format("+0x%02X -> 0x%08X", off, f));
            if (f >= 0x00400000L && f < 0x00590000L) decompile(f, d);
        }
    }

    @Override
    public void run() throws Exception {
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);

        decompile(0x0044EE20L, d);
        vtable("AI node from 0x0044EE20", 0x005E1FBCL, d);

        dumpLinear(0x004676F0L, 120);
        dumpLinear(0x00467C60L, 120);
        dumpLinear(0x00468590L, 180);

        decompile(0x004676F0L, d);
        decompile(0x00467C60L, d);
        decompile(0x00468590L, d);
        d.dispose();
    }
}
