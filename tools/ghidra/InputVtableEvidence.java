import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.listing.Function;
public class InputVtableEvidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 void dec(long x)throws Exception{
   Function f=getFunctionContaining(toAddr(x));
   println(String.format("\n=== DEC 0x%08X ===",x));
   if(f==null){println("NO_FUNCTION");return;}
   println("FUNCTION="+f.getName()+" ENTRY="+f.getEntryPoint());
   DecompileResults r=d.decompileFunction(f,90,monitor);
   if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 public void run()throws Exception{
   d.openProgram(currentProgram);
   Memory m=currentProgram.getMemory();
   long base=0x005f3ba0L;
   for(int off=0;off<=0xe0;off+=4){
     long p=Integer.toUnsignedLong(m.getInt(toAddr(base+off)));
     println(String.format("VT +0x%02X -> 0x%08X",off,p));
     if(off==0xc8) dec(p);
   }
   d.dispose();
 }
}