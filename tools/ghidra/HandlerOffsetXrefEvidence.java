import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.scalar.Scalar;

public class HandlerOffsetXrefEvidence extends GhidraScript {
    @Override public void run() throws Exception {
        long[] targets = {0x4FCL, 0x640L};
        Listing listing = currentProgram.getListing();
        InstructionIterator it = listing.getInstructions(true);
        while (it.hasNext()) {
            Instruction ins = it.next();
            boolean hit = false;
            for (int op = 0; op < ins.getNumOperands() && !hit; op++) {
                for (Object obj : ins.getOpObjects(op)) {
                    if (obj instanceof Scalar) {
                        long v = ((Scalar)obj).getUnsignedValue();
                        for (long t : targets) {
                            if (v == t) { hit = true; break; }
                        }
                    }
                }
            }
            if (hit) {
                Function f = getFunctionContaining(ins.getAddress());
                println(String.format("%s  %-28s  %s",
                    ins.getAddress(),
                    f == null ? "<no-fn>" : f.getName(),
                    ins.toString()));
            }
        }
    }
}