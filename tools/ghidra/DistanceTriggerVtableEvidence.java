import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import java.util.LinkedHashSet;
import java.util.Set;

public class DistanceTriggerVtableEvidence extends GhidraScript {
    private long ptr(long a) throws Exception { return Integer.toUnsignedLong(getInt(toAddr(a))); }
    @Override public void run() throws Exception {
        long vt=0x005EC358L;
        DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
        Set<Long> fs=new LinkedHashSet<>();
        println("VTABLE 0x005EC358");
        for(int off=0; off<0x120; off+=4){
            long p=ptr(vt+off);
            println(String.format("+0x%03X -> 0x%08X",off,p));
            if(p>=0x00400000L&&p<0x00590000L) fs.add(p);
        }
        for(long p:fs){
            Function f=getFunctionContaining(toAddr(p));
            if(f==null) continue;
            DecompileResults r=d.decompileFunction(f,60,monitor);
            if(r.decompileCompleted()&&r.getDecompiledFunction()!=null){
                String c=r.getDecompiledFunction().getC();
                if(c.contains("0x6c")||c.contains("+ 0x6c")||c.contains("0x68")||c.contains("+ 0x68")){
                    println(String.format("\n=== +? 0x%08X %s ===",p,f.getName()));
                    println(c);
                }
            }
        }
        d.dispose();
    }
}