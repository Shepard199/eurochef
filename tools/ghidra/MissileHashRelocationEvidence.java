import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.scalar.Scalar;

public class MissileHashRelocationEvidence extends GhidraScript {
    @Override public void run() throws Exception {
        long[] needles = {0x57000000L,0x57000001L,0x57000008L,0x5700000EL};
        Instruction ins = currentProgram.getListing().getInstructions(true).next();
        int hits = 0;
        while (ins != null) {
            boolean matched = false;
            for (int op = 0; op < ins.getNumOperands() && !matched; op++) {
                for (Object obj : ins.getOpObjects(op)) {
                    if (obj instanceof Scalar) {
                        long value = ((Scalar)obj).getUnsignedValue();
                        for (long needle : needles) {
                            if (value == needle) {
                                println(ins.getAddress()+" "+ins+" immediate=0x"+Long.toHexString(value));
                                matched = true;
                                hits++;
                                break;
                            }
                        }
                    }
                }
            }
            ins = ins.getNext();
        }
        println("hits="+hits);
    }
}
