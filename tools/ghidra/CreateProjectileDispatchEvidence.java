import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.scalar.Scalar;
import java.util.LinkedHashSet;
import java.util.Set;

public class CreateProjectileDispatchEvidence extends GhidraScript {
    private void dump(long raw, DecompInterface d) throws Exception {
        Function f=getFunctionContaining(toAddr(raw));
        println(String.format("\n=== 0x%08X ===",raw));
        if(f==null){println("NOFUNC"); return;}
        println("FUNCTION="+f.getName()+" ENTRY="+f.getEntryPoint());
        DecompileResults r=d.decompileFunction(f,60,monitor);
        if(r.decompileCompleted()&&r.getDecompiledFunction()!=null) println(r.getDecompiledFunction().getC());
    }
    @Override public void run() throws Exception {
        DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
        dump(0x004571E0L,d);
        dump(0x0044F180L,d);
        dump(0x00456B30L,d);
        Set<Function> funcs=new LinkedHashSet<>();
        long needle=0x1600001FL;
        println("\n=== REFS/SCALARS 0x1600001F ===");
        for(Instruction ins: currentProgram.getListing().getInstructions(true)){
            boolean hit=false;
            for(int op=0;op<ins.getNumOperands()&&!hit;op++) for(Object obj:ins.getOpObjects(op)){
                if(obj instanceof Scalar && ((Scalar)obj).getUnsignedValue()==needle){hit=true;break;}
            }
            if(!hit) continue;
            Function f=getFunctionContaining(ins.getAddress());
            println(ins.getAddress()+" "+ins+" | "+(f==null?"NOFUNC":f.getName()+"@"+f.getEntryPoint()));
            if(f!=null) funcs.add(f);
        }
        for(Function f:funcs){
            println("\n=== REF FUNC "+f.getName()+" @ "+f.getEntryPoint()+" ===");
            DecompileResults r=d.decompileFunction(f,60,monitor);
            if(r.decompileCompleted()&&r.getDecompiledFunction()!=null) println(r.getDecompiledFunction().getC());
        }
        d.dispose();
    }
}
