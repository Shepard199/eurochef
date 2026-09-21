import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;

public class PiranhaAttackSetupEvidence extends GhidraScript {
    private long ptr(long a)throws Exception{return Integer.toUnsignedLong(getInt(toAddr(a)));}
    private void dump(long raw,DecompInterface d)throws Exception{
        Function f=getFunctionAt(toAddr(raw));
        if(f==null){disassemble(toAddr(raw));try{f=createFunction(toAddr(raw),null);}catch(Exception ignored){}}
        if(f==null)f=getFunctionContaining(toAddr(raw));
        println(String.format("\n=== 0x%08X %s ===",raw,f==null?"<missing>":f.getName()));
        if(f==null)return;
        DecompileResults r=d.decompileFunction(f,90,monitor);
        if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
    }
    @Override public void run()throws Exception{
        long vt=0x005E1FBCL;
        DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
        for(int off=0;off<=0x40;off+=4){
            long f=ptr(vt+off);
            println(String.format("slot +0x%02X -> 0x%08X",off,f));
            if(off==0x20||off==0x24||off==0x28||off==0x2c||off==0x30||off==0x34||off==0x38||off==0x3c||off==0x40) dump(f,d);
        }
        dump(0x00456E60L,d);
        dump(0x00456EC0L,d);
        d.dispose();
    }
}
