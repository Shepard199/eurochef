import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.scalar.Scalar;
import java.util.LinkedHashSet;
import java.util.Set;

public class AiAttackActionStorageEvidence extends GhidraScript {
    @Override public void run() throws Exception {
        Set<Function> funcs = new LinkedHashSet<>();
        for (Instruction ins : currentProgram.getListing().getInstructions(true)) {
            boolean hit = false;
            for (int op=0; op<ins.getNumOperands() && !hit; op++) {
                for (Object obj : ins.getOpObjects(op)) {
                    if (obj instanceof Scalar) {
                        long v=((Scalar)obj).getUnsignedValue();
                        if (v==0x4b8L || v==0x4bcL) { hit=true; break; }
                    }
                }
            }
            if (!hit) continue;
            Function f=getFunctionContaining(ins.getAddress());
            println(ins.getAddress()+" "+ins+" | "+(f==null?"NOFUNC":f.getName()+"@"+f.getEntryPoint()));
            if (f!=null) funcs.add(f);
        }
        println("FUNCTIONS="+funcs.size());
    }
}
