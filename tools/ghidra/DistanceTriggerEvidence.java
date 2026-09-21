import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import java.util.LinkedHashSet;
import java.util.Set;

public class DistanceTriggerEvidence extends GhidraScript {
    @Override
    public void run() throws Exception {
        DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
        Function ctor=getFunctionContaining(toAddr(0x0048CCA0L));
        if(ctor!=null){ println("=== CTOR ==="); DecompileResults r=d.decompileFunction(ctor,60,monitor); if(r.decompileCompleted()) println(r.getDecompiledFunction().getC()); }
        Set<Function> fs=new LinkedHashSet<>();
        InstructionIterator it=currentProgram.getListing().getInstructions(true);
        while(it.hasNext()){
            Instruction ins=it.next();
            if(!ins.toString().toLowerCase().contains("0x6c")) continue;
            Function f=getFunctionContaining(ins.getAddress());
            if(f==null) continue;
            long e=f.getEntryPoint().getOffset();
            if(e>=0x0048C000L && e<0x0048E000L) fs.add(f);
        }
        for(Function f:fs){ println("\n=== "+f.getName()+" "+f.getEntryPoint()+" ==="); DecompileResults r=d.decompileFunction(f,60,monitor); if(r.decompileCompleted()) println(r.getDecompiledFunction().getC()); }
        d.dispose();
    }
}