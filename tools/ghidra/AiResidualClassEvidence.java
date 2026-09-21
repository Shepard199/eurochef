import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;

public class AiResidualClassEvidence extends GhidraScript {
    private void decompile(long raw, DecompInterface d) throws Exception {
        Address a = toAddr(raw);
        Function f = getFunctionContaining(a);
        println(String.format("\n=== FUNC 0x%08X ===", raw));
        if (f == null) { println("NO FUNCTION"); return; }
        println("ENTRY=" + f.getEntryPoint() + " NAME=" + f.getName());
        DecompileResults r = d.decompileFunction(f, 60, monitor);
        if (r.decompileCompleted() && r.getDecompiledFunction() != null) {
            println(r.getDecompiledFunction().getC());
        }
    }

    private void dumpDescriptor(String name, long raw) throws Exception {
        Address a = toAddr(raw);
        println(String.format("\n=== DESCRIPTOR %s 0x%08X ===", name, raw));
        for (int off = 0; off < 0x30; off += 4) {
            long v = Integer.toUnsignedLong(getInt(a.add(off)));
            println(String.format("+0x%02X = 0x%08X", off, v));
        }
        for (Reference ref : getReferencesTo(a)) {
            Function f = getFunctionContaining(ref.getFromAddress());
            println("REF " + ref.getFromAddress() + " " + ref.getReferenceType() + " " +
                (f == null ? "NOFUNC" : f.getName() + "@" + f.getEntryPoint()));
        }
    }

    @Override
    public void run() throws Exception {
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        decompile(0x0047EA70L, d);
        decompile(0x004190D0L, d);
        dumpDescriptor("MonsterBase", 0x005E260CL);
        dumpDescriptor("Em07PiranhaBot", 0x005E28ECL);
        dumpDescriptor("TestAnimBot", 0x005E28FCL);
        d.dispose();
    }
}
