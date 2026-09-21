import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.scalar.Scalar;

public class MissileSpreadsheetEvidence extends GhidraScript {
    @Override public void run() throws Exception {
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        for (Instruction ins : currentProgram.getListing().getInstructions(true)) {
            boolean hit = false;
            for (int op = 0; op < ins.getNumOperands(); op++) {
                for (Object obj : ins.getOpObjects(op)) {
                    if (obj instanceof Scalar) {
                        long value = ((Scalar)obj).getUnsignedValue();
                        if (value == 0x1400000fL) hit = true;
                    }
                }
            }
            if (!hit) continue;
            Function f = getFunctionContaining(ins.getAddress());
            println("REF " + ins.getAddress() + " " + ins + " FUNCTION=" + (f == null ? "<none>" : f.getName()));
            Instruction cur = ins;
            for (int back = 0; back < 24 && cur.getPrevious() != null; back++) cur = cur.getPrevious();
            for (int n = 0; n < 80 && cur != null; n++, cur = cur.getNext()) {
                println(cur.getAddress() + " " + cur);
            }
            if (f != null) {
                DecompileResults r = d.decompileFunction(f, 60, monitor);
                if (r.decompileCompleted() && r.getDecompiledFunction() != null) {
                    println(r.getDecompiledFunction().getC());
                }
            }
        }
        d.dispose();
    }
}
