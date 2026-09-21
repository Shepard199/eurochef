import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.Function;
import ghidra.program.model.scalar.Scalar;

public class SetupIdleEventRefs extends GhidraScript {
    @Override public void run() throws Exception {
        long needle = 0x16000001L;
        int hits = 0;
        for (Instruction ins : currentProgram.getListing().getInstructions(true)) {
            boolean match = false;
            for (int op = 0; op < ins.getNumOperands() && !match; op++) {
                for (Object obj : ins.getOpObjects(op)) {
                    if (obj instanceof Scalar) {
                        long value = ((Scalar)obj).getUnsignedValue();
                        if (value == needle) { match = true; break; }
                    }
                }
            }
            if (!match) continue;
            Function f = getFunctionContaining(ins.getAddress());
            println(ins.getAddress() + " " + ins + " | " +
                (f == null ? "NOFUNC" : f.getName() + "@" + f.getEntryPoint()));
            hits++;
        }
        println("HITS=" + hits);
    }
}
